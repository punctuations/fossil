const MODEL_ID_BITS: f64 = 8.0;
const LENGTH_FIELD_BITS: f64 = 32.0;

pub fn histogram(bytes: &[u8]) -> [usize; 256] {
    let mut count: [usize; 256] = [0; 256];

    for b in bytes.iter() {
        count[*b as usize] += 1;
    }

    return count;
}

/// unique byte count
pub fn ubytes(bytes: &[u8]) -> usize {
    histogram(&bytes).iter().filter(|c| **c > 0).count()
}

/// https://en.wikipedia.org/wiki/Entropy_(information_theory)
pub fn shannon(bytes: &[u8]) -> f64 {
    if bytes.is_empty() { return 0.0; }

    let total = bytes.len() as f64;
    let counts = histogram(bytes);

    let mut entropy = 0.0;
    for &count in counts.iter() {
        if count == 0 { continue; }

        let p = count as f64 / total;
        entropy -= p * p.log2();
    }

    return entropy;
}

/// estimated entropy cost bits
pub fn cost(bytes: &[u8]) -> f64 {
    bytes.len() as f64 * shannon(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyClass {
    Empty,
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

impl EntropyClass {
    pub fn classify(len: usize, bits_per_byte: f64) -> Self {
        if len == 0 { return EntropyClass::Empty; }

        return match bits_per_byte {
            b if b < 1.0 => EntropyClass::VeryLow,
            b if b < 3.0 => EntropyClass::Low,
            b if b < 5.0 => EntropyClass::Medium,
            b if b < 7.0 => EntropyClass::High,
            _            => EntropyClass::VeryHigh,
        };
    }

    pub fn label(&self) -> &'static str {
        return match self {
            EntropyClass::Empty => "empty",
            EntropyClass::VeryLow => "very low",
            EntropyClass::Low => "low",
            EntropyClass::Medium => "medium",
            EntropyClass::High => "high",
            EntropyClass::VeryHigh => "very high"
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntropyReport {
    pub len: usize,
    /// entropy bits per byte
    pub entropy_bpb: f64,
    /// estimated cost bits
    pub estimated_cost: f64,
    /// raw cost bits
    pub raw_cost: usize,
    pub unique_bytes: usize,
    pub compressibility_score: f64,
    pub class: EntropyClass,
}

pub fn analyze(bytes: &[u8]) -> EntropyReport {
    let entropy = shannon(bytes);
    let unique = ubytes(bytes);
    let len = bytes.len();

    return EntropyReport { 
        len: len, 
        entropy_bpb: entropy, 
        estimated_cost: len as f64 * entropy, 
        raw_cost: len * 8, 
        unique_bytes: unique, 
        compressibility_score: if len == 0 { 0.0} else {(1.0 - entropy / 8.0).clamp(0.0, 1.0)}, 
        class: EntropyClass::classify(len, entropy) }
}

pub struct ChunkEntropyReport {
    /// byte pos of this chunk in original file
    pub offset: usize,
    /// 0-based chunk number
    pub index: usize,
    pub report: EntropyReport
}

pub fn analyze_chunks(bytes: &[u8], chunk_size: usize) -> Vec<ChunkEntropyReport>{
    if bytes.is_empty() { return Vec::new(); }

    let step = if chunk_size == 0 { bytes.len() } else { chunk_size };

    return bytes
        .chunks(step)
        .enumerate()
        .map(|(index, chunk)| ChunkEntropyReport {
            offset: index * step,
            index,
            report: analyze(chunk),
        })
        .collect();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateModel {
    Raw,
    Solid,
    Rle,
    Copy,
    Delta,
    Dict,
    Entropy
}

impl CandidateModel {
    pub fn name(&self) -> &'static str {
        return match self {
            CandidateModel::Raw => "RAW",
            CandidateModel::Solid => "SOLID",
            CandidateModel::Rle => "RLE",
            CandidateModel::Copy => "COPY",
            CandidateModel::Delta => "DELTA",
            CandidateModel::Dict => "DICTIONARY",
            CandidateModel::Entropy => "ENTROPY"
        }
    }
}

pub struct ModelCost {
    pub model: CandidateModel,
    pub estimated_bits: f64, // f64::INFINITY == 'not applicable'
    pub reason: String,
}

impl ModelCost {
    fn is_applicable(&self) -> bool {
        self.estimated_bits.is_finite()
    }
}

pub struct ModelSelection {
    pub selected: CandidateModel,
    pub estimated_bits: f64,
    pub candidates: Vec<ModelCost>
}

fn estimate_raw(bytes: &[u8]) -> ModelCost {
    return ModelCost {
        model: CandidateModel::Raw,
        estimated_bits: MODEL_ID_BITS + 8.0 * bytes.len() as f64,
        reason: "store bytes directly (fallback)".to_string()
    };
}

fn estimate_solid(bytes: &[u8]) -> ModelCost {
    if bytes.is_empty() {
        return ModelCost {
            model: CandidateModel::Solid,
            estimated_bits: f64::INFINITY,
            reason: "empty is a better model".to_string()
        }
    } else {
        if bytes.iter().all(|b| *b == bytes[0]) {
            return ModelCost {
                model: CandidateModel::Solid,
                estimated_bits: MODEL_ID_BITS + LENGTH_FIELD_BITS + 8.0,
                reason: "bytes are uniform".to_string()
            }
        } else {
            return ModelCost {
                model: CandidateModel::Solid,
                estimated_bits: f64::INFINITY,
                reason: "not applicable: bytes are not uniform".to_string()
            }
        }
    }
}

fn estimate_rle(bytes: &[u8]) -> ModelCost {
    fn count_runs(bytes: &[u8]) -> usize {
        let mut count = 1;
        let mut p = bytes[0];
        for &b in &bytes[1..] {
            if b != p {
                count += 1;
                p = b;
            }
        }

        return count;
    }

    return ModelCost {
        model: CandidateModel::Rle,
        estimated_bits: if bytes.len() == 0 { f64::INFINITY } else { MODEL_ID_BITS + count_runs(bytes) as f64 * (LENGTH_FIELD_BITS + 8.0) },
        reason: "".to_string()
    }
}

fn estimate_entropy(bytes: &[u8]) -> ModelCost {
    return ModelCost {
        model: CandidateModel::Entropy,
        estimated_bits: if bytes.len() == 0 { f64::INFINITY } else { MODEL_ID_BITS + cost(bytes) },
        reason: "estimated from Shannon entropy".to_string()
    }
}

fn stub(model: CandidateModel) -> ModelCost {
    return ModelCost { model, estimated_bits: f64::INFINITY, reason: "not implemented yet".to_string() };
}

pub fn estimate_all(bytes: &[u8]) -> Vec<ModelCost> {
    return vec![
        estimate_raw(bytes),
        estimate_solid(bytes),
        estimate_rle(bytes),
        estimate_entropy(bytes),
        stub(CandidateModel::Dict),
        stub(CandidateModel::Delta),
        stub(CandidateModel::Copy)
    ]
}

pub fn select_best_model(bytes: &[u8]) -> ModelSelection {
    let candidates = estimate_all(bytes);

    let best = candidates
        .iter()
        .filter(|c| c.is_applicable())
        .min_by(|a, b| a.estimated_bits.total_cmp(&b.estimated_bits))
        .expect("RAW is always acceptable");

    return ModelSelection {
        selected: best.model,
        estimated_bits: best.estimated_bits,
        candidates,
    }
}