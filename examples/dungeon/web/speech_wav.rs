//! Original strict PCM WAV reader/assembler for buffered native voice parts.
use super::*;
use gamectx::obj;
const CAP: usize = providers::MAX_BYTES * 3 / 4 - 4096;
pub struct Pcm<'a> {
    pub rate: u32,
    pub data: &'a [u8],
}
pub fn pcm(bytes: &[u8]) -> Result<Pcm<'_>> {
    if bytes.len() < 44 || bytes.len() > CAP || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(err());
    }
    let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    if u32_at(4) as usize != bytes.len() - 8 {
        return Err(err());
    }
    let mut pos = 12;
    let mut rate = None;
    let mut data = None;
    let mut count = 0;
    while pos < bytes.len() {
        count += 1;
        if count > 64 || bytes.len() - pos < 8 {
            return Err(err());
        }
        let size = u32_at(pos + 4) as usize;
        let begin = pos + 8;
        let end = begin.checked_add(size).filter(|&e| e <= bytes.len()).ok_or_else(err)?;
        match &bytes[pos..pos + 4] {
            b"fmt " => {
                if rate.is_some() || !(16..=40).contains(&size) {
                    return Err(err());
                }
                let p = &bytes[begin..end];
                let n = u32::from_le_bytes(p[4..8].try_into().unwrap());
                if p[..4] != [1, 0, 1, 0]
                    || p[12..16] != [2, 0, 16, 0]
                    || !(8000..=96000).contains(&n)
                    || u32::from_le_bytes(p[8..12].try_into().unwrap()) != n * 2
                {
                    return Err(err());
                }
                rate = Some(n);
            }
            b"data" => {
                if data.is_some() || size == 0 || size % 2 != 0 {
                    return Err(err());
                }
                data = Some(&bytes[begin..end]);
            }
            _ => {}
        }
        pos = end + (size % 2);
        if pos > bytes.len() {
            return Err(err());
        }
    }
    Ok(Pcm { rate: rate.ok_or_else(err)?, data: data.ok_or_else(err)? })
}
pub fn join(parts: &[(Vec<u8>, u64)]) -> Result<Value> {
    if parts.is_empty() || parts.len() > 240 {
        return Err(err());
    }
    let mut rate = 0;
    let mut data = Vec::new();
    let mut boundaries = Vec::new();
    let mut last = 0;
    for (bytes, through) in parts {
        let p = pcm(bytes)?;
        if (rate != 0 && rate != p.rate) || *through <= last || *through > 240 || data.len() + p.data.len() + 44 > CAP {
            return Err(err());
        }
        rate = p.rate;
        data.extend_from_slice(p.data);
        last = *through;
        boundaries.push(obj! {"frame"=>data.len()/2,"through"=>*through});
    }
    let mut bytes = Vec::with_capacity(data.len() + 44);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((data.len() + 36) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&[1, 0, 1, 0]);
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&[2, 0, 16, 0]);
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    Ok(obj! {"audio"=>providers::encode64(&bytes),"sample_rate"=>rate,"boundaries"=>boundaries})
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wav(rate: u32) -> Vec<u8> {
        let mut b = b"RIFF".to_vec();
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&[1, 0, 1, 0]);
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2).to_le_bytes());
        b.extend_from_slice(&[2, 0, 16, 0]);
        b.extend_from_slice(b"data");
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(&[1, 0, 2, 0]);
        b
    }
    #[test]
    fn joins_pcm_with_measured_boundaries() {
        let v = join(&[(wav(22050), 8), (wav(22050), 21)]).unwrap();
        let b = providers::decode64(text(&v, "audio").unwrap()).unwrap();
        assert_eq!(pcm(&b).unwrap().data, &[1, 0, 2, 0, 1, 0, 2, 0]);
        assert_eq!(
            v.get("boundaries").unwrap(),
            &vec![obj! {"frame"=>2,"through"=>8}, obj! {"frame"=>4,"through"=>21}].into()
        );
    }
    #[test]
    fn rejects_broken_or_incompatible_audio() {
        let b = wav(22050);
        for n in 0..b.len() {
            assert!(pcm(&b[..n]).is_err());
        }
        assert!(join(&[(b.clone(), 4), (wav(16000), 10)]).is_err());
        assert!(join(&[(b.clone(), 4), (b.clone(), 4)]).is_err());
        let mut bad = b.clone();
        bad[20] = 3;
        assert!(pcm(&bad).is_err());
        let mut bad = b;
        bad[40] = 6;
        assert!(pcm(&bad).is_err());
    }
    #[test]
    fn mutated_headers_never_panic() {
        let seed = wav(22050);
        for n in 0..2000 {
            let mut bytes = seed.clone();
            let at = n % bytes.len();
            bytes[at] = (n % 256) as u8;
            let _ = pcm(&bytes);
            let _ = join(&[(bytes, 16)]);
        }
    }
}
