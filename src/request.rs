//! Full-request accounting supplied by the host's actual renderer/tokenizer. This module
//! does not tokenize HTTP JSON or assume that provider-native tools cost zero tokens.
use crate::budget::{self, Packet, PreparedSection, Section};
use crate::{checked as c, obj, Error, Result, Value};
use std::cell::RefCell;

mod composition;
pub use composition::Composition;

pub const MAX_REQUEST_BYTES: usize = 1_572_864;
pub const MAX_TOKENS: usize = 1_048_576;
/// Whether a host measured the full rendered prompt or estimated it. Exactness is a
/// host assertion; the library cannot verify an external tokenizer or hidden template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountMethod {
    Exact,
    Estimate,
}
impl CountMethod {
    pub fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Estimate => "estimate",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Count {
    pub input: usize,
    pub method: CountMethod,
}
impl Count {
    pub fn from_value(v: &Value) -> Result<Self> {
        c::fields(v, &["input", "method"])?;
        let method = match c::text(c::get(v, "method")?)? {
            "exact" => CountMethod::Exact,
            "estimate" => CountMethod::Estimate,
            _ => return Err(c::bad()),
        };
        Ok(Self { input: c::uint(c::get(v, "input")?, 0, MAX_TOKENS as u64)? as usize, method })
    }
}
/// Explicit context/output/safety contract. No context size is a universal default.
#[derive(Clone, Copy, Debug)]
pub struct RequestBudget {
    context: usize,
    output: usize,
    safety: usize,
}
#[derive(Clone, Debug)]
pub struct RequestReceipt {
    pub input: usize,
    pub remaining: usize,
    pub overflow: usize,
    pub method: CountMethod,
    budget: RequestBudget,
}
impl RequestReceipt {
    pub fn fits(&self) -> bool {
        self.overflow == 0
    }
    pub fn to_value(&self) -> Value {
        obj! {"context"=>self.budget.context,"input"=>self.input,"reserved_output"=>self.budget.output,
        "safety"=>self.budget.safety,"remaining"=>self.remaining,"overflow"=>self.overflow,
        "method"=>self.method.name(),"fits"=>self.fits()}
    }
}
#[derive(Clone, Debug)]
pub struct BudgetedRequest {
    pub packet: Packet,
    pub request: Value,
    pub receipt: RequestReceipt,
}
impl BudgetedRequest {
    pub fn to_value(&self) -> Value {
        obj! {"packet"=>self.packet.value.clone(),"request"=>self.request.clone(),"sections"=>self.packet.receipt.to_value(),"budget"=>self.receipt.to_value()}
    }
}
pub(crate) fn bounded(v: &Value, cap: usize) -> Result<()> {
    if v.is_valid(cap) {
        Ok(())
    } else {
        Err(c::bad())
    }
}
impl RequestBudget {
    pub fn new(context: usize, output: usize, safety: usize) -> Result<Self> {
        if context == 0 || context > MAX_TOKENS || output > context || safety > context - output {
            return Err(c::bad());
        }
        Ok(Self { context, output, safety })
    }
    pub fn from_value(v: &Value) -> Result<Self> {
        c::fields(v, &["context", "output", "safety"])?;
        Self::new(
            c::uint(c::get(v, "context")?, 1, MAX_TOKENS as u64)? as usize,
            c::uint(c::get(v, "output")?, 0, MAX_TOKENS as u64)? as usize,
            c::uint(c::get(v, "safety")?, 0, MAX_TOKENS as u64)? as usize,
        )
    }
    pub fn to_value(&self) -> Value {
        obj! {"context"=>self.context,"output"=>self.output,"safety"=>self.safety}
    }
    /// Reports overflow without truncating or disguising estimated counts as exact.
    pub fn assess(&self, count: Count) -> Result<RequestReceipt> {
        if count.input > MAX_TOKENS {
            return Err(c::bad());
        }
        let used = count.input + self.output + self.safety;
        Ok(RequestReceipt {
            input: count.input,
            remaining: self.context.saturating_sub(used),
            overflow: used.saturating_sub(self.context),
            method: count.method,
            budget: *self,
        })
    }
    /// Counter receives the complete provider request, including tools, format and chat
    /// history. The host must render/tokenize using the exact deployed template. Recheck
    /// after every tool response/correction, and enforce the reserved output at generation.
    pub fn check(&self, request: &Value, mut counter: impl FnMut(&Value) -> Result<Count>) -> Result<RequestReceipt> {
        bounded(request, MAX_REQUEST_BYTES)?;
        let receipt = self.assess(counter(request)?)?;
        if !receipt.fits() {
            return Err(Error::new("complete request exceeds context budget"));
        }
        Ok(receipt)
    }
    /// Reuses legacy stable ordering and priority dropping; required sections never drop.
    /// Counter/renderer errors abort the build, rather than causing content to be removed.
    pub fn assemble(
        &self,
        sections: Vec<Section>,
        renderer: impl FnMut(&Value) -> Result<Value>,
        counter: impl FnMut(&Value) -> Result<Count>,
    ) -> Result<BudgetedRequest> {
        let prepared = sections.into_iter().map(PreparedSection::new).collect::<Vec<_>>();
        self.assemble_prepared(&prepared.iter().collect::<Vec<_>>(), renderer, counter)
    }
    pub fn assemble_prepared(
        &self,
        sections: &[&PreparedSection],
        renderer: impl FnMut(&Value) -> Result<Value>,
        counter: impl FnMut(&Value) -> Result<Count>,
    ) -> Result<BudgetedRequest> {
        if sections.len() > 128
            || sections.iter().map(|s| s.section().value.json_len()).sum::<usize>() > MAX_REQUEST_BYTES
        {
            return Err(c::bad());
        }
        let renderer = RefCell::new(renderer);
        let counter = RefCell::new(counter);
        let error = RefCell::new(None);
        let selected = RefCell::new(None);
        let packet = budget::assemble_prepared(sections, &|text| {
            if error.borrow().is_some() {
                return false;
            }
            let result = (|| {
                let value = crate::json::parse(text, MAX_REQUEST_BYTES)?;
                let request = renderer.borrow_mut()(&value)?;
                bounded(&request, MAX_REQUEST_BYTES)?;
                let receipt = self.assess(counter.borrow_mut()(&request)?)?;
                if receipt.fits() {
                    *selected.borrow_mut() = Some((request, receipt));
                    return Ok(true);
                }
                Ok(false)
            })();
            match result {
                Ok(fits) => fits,
                Err(e) => {
                    *error.borrow_mut() = Some(e);
                    false
                }
            }
        });
        if let Some(e) = error.into_inner() {
            return Err(e);
        }
        let packet = packet?;
        let (request, receipt) = selected.into_inner().ok_or_else(c::bad)?;
        Ok(BudgetedRequest { packet, request, receipt })
    }
}
