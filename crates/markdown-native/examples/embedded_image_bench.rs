//! Reproduce decoder placement/cost without codecs, GTK or a network service.
use base64::Engine;
use std::{hint::black_box, sync::Arc, time::Instant};

type Decode<'a> = Box<dyn Fn() -> Arc<[u8]> + 'a>;
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mib = args
        .first()
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(10);
    let iterations = args
        .get(1)
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(7);
    assert!((1..=10).contains(&mib) && (1..=100).contains(&iterations));
    let bytes = vec![42; mib * 1024 * 1024];
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let uri = format!("data:image/png;base64,{encoded}");
    let cases: [(&str, Decode<'_>); 3] = [
        (
            "existing-base64",
            Box::new(|| {
                base64::engine::general_purpose::STANDARD
                    .decode(black_box(&encoded))
                    .unwrap()
                    .into()
            }),
        ),
        (
            "standards-data-url",
            Box::new(|| {
                data_url::DataUrl::process(black_box(&uri))
                    .unwrap()
                    .decode_to_vec()
                    .unwrap()
                    .0
                    .into()
            }),
        ),
        (
            "bounded-shared",
            Box::new(|| {
                studio_markdown_native::embedded_image::decode(black_box(&uri), bytes.len())
                    .unwrap()
                    .bytes
            }),
        ),
    ];
    // Warm every case and rotate their order to reduce allocator/cache and
    // CPU-frequency bias. Each path returns the same immutable byte type.
    for (_, decode) in &cases {
        assert_eq!(black_box(decode()).as_ref(), bytes);
    }
    let mut samples: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
    for pass in 0..iterations {
        for step in 0..cases.len() {
            let index = (pass + step) % cases.len();
            let at = Instant::now();
            let decoded = black_box(cases[index].1());
            samples[index].push(at.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(decoded.as_ref(), bytes);
        }
    }
    println!("decoder,decoded_bytes,encoded_bytes,iterations,mean_ms,min_ms,median_ms");
    for ((name, _), mut times) in cases.into_iter().zip(samples) {
        times.sort_by(f64::total_cmp);
        println!(
            "{name},{},{},{iterations},{:.3},{:.3},{:.3}",
            bytes.len(),
            encoded.len(),
            times.iter().sum::<f64>() / iterations as f64,
            times[0],
            times[iterations / 2]
        );
    }
}
