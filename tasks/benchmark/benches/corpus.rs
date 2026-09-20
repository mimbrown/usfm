//! Throughput benchmarks over the committed corpus (`corpus/`, see its
//! README). Every group reports throughput — criterion prints it in MiB/s —
//! and the input of one benchmark id is a whole file class, lexed or parsed
//! one file after another in a single iteration, so the number is an aggregate
//! over the class rather than one book.
//!
//! Run with `cargo bench -p usfm_benchmark`. The recorded baseline, the
//! machine it was taken on and what counts as a regression are in
//! `docs/benchmarks.md`.
//!
//! The `lex` group reaches `Lexer::new`, which takes a `UniquePromise`,
//! through the parser's `benchmarking` feature: it is what re-exports the type
//! and its `new_for_tests_and_benchmarks` constructor. This crate turns it on
//! through the facade's feature of the same name, in its `Cargo.toml`.

use std::hint::black_box;
use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use usfm::ast::Document;
use usfm::html::to_html_string;
use usfm::json::to_json_string;
use usfm::parser::lexer::Lexer;
use usfm::parser::parser::Parser;
use usfm::parser::{DEFAULT_STYLESHEET, UniquePromise};
use usfm::semantic::ReferenceIndex;
use usfm::style::StyleSheet;
use usfm::usx::to_usx_string;
use usfm_benchmark::{CorpusFile, FileClass, total_bytes};

/// Criterion settings chosen so a full `cargo bench -p usfm_benchmark`
/// finishes in a few minutes on a 4-vCPU VM: the whole-corpus input is 12.8 MB
/// and one iteration of it is a large fraction of a second, so the default
/// 100 samples over 5 s would take hours.
fn configured() -> Criterion {
    Criterion::default()
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(5))
        .sample_size(10)
        .configure_from_args()
}

/// The default stylesheet, built once for every group.
fn style_sheet() -> Arc<StyleSheet> {
    Arc::clone(&DEFAULT_STYLESHEET)
}

/// Load every class once, so no group pays for disk in a timed loop.
fn corpus() -> Vec<(FileClass, Vec<CorpusFile>)> {
    FileClass::ALL
        .into_iter()
        .map(|class| (class, class.load()))
        .collect()
}

/// One group per pipeline stage: `run` is given a class's files and the
/// stylesheet, and does one iteration's worth of work over all of them.
fn throughput_group<F>(c: &mut Criterion, name: &str, run: F)
where
    F: Fn(&[CorpusFile], &Arc<StyleSheet>),
{
    let sheet = style_sheet();
    let mut group = c.benchmark_group(name);
    for (class, files) in corpus() {
        group.throughput(Throughput::Bytes(total_bytes(&files)));
        group.bench_with_input(
            BenchmarkId::from_parameter(class.name()),
            &files,
            |b, files| b.iter(|| run(files, &sheet)),
        );
    }
    group.finish();
}

/// The lexer alone, driven to exhaustion. `UniquePromise` says only one
/// `Lexer` exists at a time on this thread, which holds: the files are lexed
/// one after another and each `Lexer` is dropped before the next is built.
fn bench_lex(c: &mut Criterion) {
    throughput_group(c, "lex", |files, _sheet| {
        for file in files {
            let lexer = Lexer::new(
                black_box(&file.text),
                UniquePromise::new_for_tests_and_benchmarks(),
            );
            for token in lexer {
                black_box(token);
            }
        }
    });
}

fn bench_parse(c: &mut Criterion) {
    throughput_group(c, "parse", |files, sheet| {
        for file in files {
            black_box(Parser::new(black_box(&file.text)).parse(sheet));
        }
    });
}

/// `usfm::parse`: the same parse as `parse` above, plus `usfm_semantic`'s walk
/// of the tree it produced and the merge of the two diagnostic lists (ticket
/// 19). The pair is what says what the semantic pass costs, which is why both
/// groups are kept. The facade parses with `DEFAULT_STYLESHEET`, the same
/// `Arc` `style_sheet()` hands the other groups, so the only difference
/// measured is the second pass.
fn bench_parse_semantic(c: &mut Criterion) {
    throughput_group(c, "parse_semantic", |files, _sheet| {
        for file in files {
            black_box(usfm::parse(black_box(&file.text)));
        }
    });
}

fn bench_parse_usx(c: &mut Criterion) {
    throughput_group(c, "parse_usx", |files, sheet| {
        for file in files {
            let result = Parser::new(black_box(&file.text)).parse(sheet);
            black_box(to_usx_string(&result.document));
        }
    });
}

fn bench_parse_html(c: &mut Criterion) {
    throughput_group(c, "parse_html", |files, sheet| {
        for file in files {
            let result = Parser::new(black_box(&file.text)).parse(sheet);
            let sheet = Arc::clone(result.document.style_sheet());
            black_box(to_html_string(&result.document, &sheet));
        }
    });
}

fn bench_parse_json(c: &mut Criterion) {
    throughput_group(c, "parse_json", |files, sheet| {
        for file in files {
            let result = Parser::new(black_box(&file.text)).parse(sheet);
            black_box(to_json_string(&result.document));
        }
    });
}

/// A group over trees built once, outside the timed loop, so the number is the
/// cost of walking a built tree. Throughput is still counted over the bytes of
/// source the trees came from, which keeps the unit the same as the groups
/// above without the two being directly comparable.
fn built_tree_group<F>(c: &mut Criterion, name: &str, run: F)
where
    F: Fn(&Document<'_>),
{
    let sheet = style_sheet();
    let mut group = c.benchmark_group(name);
    for (class, files) in corpus() {
        let bytes = total_bytes(&files);
        let documents: Vec<Document<'_>> = files
            .iter()
            .map(|file| Parser::new(&file.text).parse(&sheet).document)
            .collect();
        group.throughput(Throughput::Bytes(bytes));
        group.bench_with_input(
            BenchmarkId::from_parameter(class.name()),
            &documents,
            |b, documents| {
                b.iter(|| {
                    for document in documents {
                        run(document);
                    }
                })
            },
        );
    }
    group.finish();
}

/// `reference_index` alone, over a tree parsed outside the timed loop.
///
/// The index moved to `usfm_semantic` in ticket 22; the call it times is the
/// same walk under a new name (`document.reference_index()` was
/// `ReferenceIndex::build`).
fn bench_reference_index(c: &mut Criterion) {
    built_tree_group(c, "reference_index", |document| {
        black_box(ReferenceIndex::new(document));
    });
}

/// `usfm_semantic::analyze` alone, over a tree parsed outside the timed loop
/// (ticket 24).
///
/// `parse_semantic` minus `parse` says what the pass costs *in place*, where
/// the tree is still warm from being built; this says what the walk and the
/// checks cost on their own, which is what an editor re-checking a cached tree
/// would pay, and it is the number to split by check family when the gap
/// between the two groups needs attributing.
fn bench_analyze(c: &mut Criterion) {
    built_tree_group(c, "analyze", |document| {
        black_box(usfm::semantic::analyze(document));
    });
}

criterion_group! {
    name = benches;
    config = configured();
    targets = bench_lex, bench_parse, bench_parse_semantic, bench_parse_usx, bench_parse_html,
        bench_parse_json, bench_reference_index, bench_analyze
}
criterion_main!(benches);
