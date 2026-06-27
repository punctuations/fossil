use fossil::core::entropy::*;

#[test]
fn empty_input_is_well_behaved() {
    let report = analyze(&[]);
    assert_eq!(report.len, 0);
    assert_eq!(report.entropy_bpb, 0.0);
    assert_eq!(report.raw_cost, 0);
    assert_eq!(report.unique_bytes, 0);
    assert_eq!(report.compressibility_score, 0.0);
    assert_eq!(report.class, EntropyClass::Empty);
    assert!(analyze_chunks(&[], 4).is_empty());
}

#[test]
fn uniform_data_has_zero_entropy() {
    let data = [0xAA_u8; 64];
    assert_eq!(shannon(&data), 0.0);
    assert_eq!(ubytes(&data), 1);

    let report = analyze(&data);
    assert_eq!(report.class, EntropyClass::VeryLow);
    assert!(report.compressibility_score > 0.99);
}

#[test]
fn all_distinct_bytes_reach_eight_bits() {
    let data: Vec<u8> = (0..=255).collect();
    assert!((shannon(&data) - 8.0).abs() < 1e-9);
    assert_eq!(ubytes(&data), 256);
    assert_eq!(analyze(&data).class, EntropyClass::VeryHigh);
}

#[test]
fn histogram_and_unique_agree() {
    let data = b"aaabbc";
    let hist = histogram(data);
    assert_eq!(hist[b'a' as usize], 3);
    assert_eq!(hist[b'b' as usize], 2);
    assert_eq!(hist[b'c' as usize], 1);
    assert_eq!(ubytes(data), 3);
}

#[test]
fn cost_matches_len_times_entropy() {
    let data = b"aaabbc";
    assert!((cost(data) - data.len() as f64 * shannon(data)).abs() < 1e-9);
}

#[test]
fn classify_thresholds() {
    assert_eq!(EntropyClass::classify(0, 0.0), EntropyClass::Empty);
    assert_eq!(EntropyClass::classify(10, 0.5), EntropyClass::VeryLow);
    assert_eq!(EntropyClass::classify(10, 2.0), EntropyClass::Low);
    assert_eq!(EntropyClass::classify(10, 4.0), EntropyClass::Medium);
    assert_eq!(EntropyClass::classify(10, 6.0), EntropyClass::High);
    assert_eq!(EntropyClass::classify(10, 7.5), EntropyClass::VeryHigh);
}

#[test]
fn chunks_track_offsets_and_remainder() {
    let chunks = analyze_chunks(b"0123456789", 4);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].offset, 0);
    assert_eq!(chunks[1].offset, 4);
    assert_eq!(chunks[2].offset, 8);
    assert_eq!(chunks[0].index, 0);
    assert_eq!(chunks[2].index, 2);
    assert_eq!(chunks[2].report.len, 2);
}

#[test]
fn chunk_size_zero_is_one_window() {
    let chunks = analyze_chunks(b"0123456789", 0);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].report.len, 10);
}

#[test]
fn raw_wins_for_incompressible_data() {
    let data: Vec<u8> = (0..=255).collect();
    assert_eq!(select_best_model(&data).selected, CandidateModel::Raw);
}

#[test]
fn uniform_data_beats_raw() {
    let data = [0x42_u8; 1024];
    let selection = select_best_model(&data);
    let raw = 8.0 + 8.0 * data.len() as f64;
    assert_ne!(selection.selected, CandidateModel::Raw);
    assert!(selection.estimated_bits < raw);
}

#[test]
fn rle_cost_reflects_run_count() {
    let three_runs = b"AAAAABBBCCCCCCCC";
    let rle = select_best_model(three_runs)
        .candidates
        .into_iter()
        .find(|c| c.model == CandidateModel::Rle)
        .unwrap();
    assert_eq!(rle.estimated_bits, 8.0 + 3.0 * (32.0 + 8.0));

    let one_run = select_best_model(&[0x42_u8; 64])
        .candidates
        .into_iter()
        .find(|c| c.model == CandidateModel::Rle)
        .unwrap();
    assert_eq!(one_run.estimated_bits, 8.0 + 1.0 * (32.0 + 8.0));
}

#[test]
fn two_runs_select_rle() {
    let mut data = vec![0x01_u8; 500];
    data.extend(std::iter::repeat(0x02_u8).take(500));
    assert_eq!(select_best_model(&data).selected, CandidateModel::Rle);
}

#[test]
fn stub_models_are_never_applicable() {
    let selection = select_best_model(b"some bytes here");
    for c in &selection.candidates {
        match c.model {
            CandidateModel::Dict | CandidateModel::Copy | CandidateModel::Delta => {
                assert!(!c.estimated_bits.is_finite());
            }
            _ => {}
        }
    }
}

#[test]
fn a_model_is_always_selected() {
    let selection = select_best_model(b"\x00\xFF\x10\x80\x01\xAB");
    assert!(selection.estimated_bits.is_finite());
    assert_eq!(selection.candidates.len(), 7);
}
