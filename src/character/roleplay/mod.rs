//! Opt-in roleplay policies for AI characters: greetings, conversation routing, expression,
//! wording, viewpoints, reactions, deliberation, stage actions, commands and message chunks.
//! Every policy proposes; the host validates, executes and confirms delivery.
mod wording;
pub use wording::{validate_wording, wording_anchors};
mod expression;
pub(crate) use expression::fixture as expression_fixture;
pub use expression::{
    choose_tale_seed, distinct_expression, expression_claim, expression_packet, expression_route,
    expression_route_expanded, tale_sentences, validate_expression,
};
mod actions;
pub(crate) use actions::fixture as roleplay_actions_fixture;
pub use actions::RoleplayActions;
mod message_chunks;
pub use message_chunks::message_chunks;
mod deliberation;
pub(crate) use deliberation::fixture as deliberation_fixture;
pub use deliberation::{
    deliberation_plan, validate_deliberation_config, validate_hypotheses, validate_thinking_values,
};
mod greetings;
pub(crate) use greetings::fixture as greetings_fixture;
pub use greetings::GreetingMemory;
mod conversation;
pub use conversation::conversation_route;
mod reactions;
pub(crate) use reactions::fixture as reactions_fixture;
pub use reactions::ReactionPolicy;
mod viewpoints;
pub(crate) use viewpoints::fixture as viewpoints_fixture;
pub use viewpoints::{validate_viewpoint_line, Viewpoints};
mod commands;
pub use commands::resolve_command;
