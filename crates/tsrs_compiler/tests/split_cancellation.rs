// EyeAuras fork (POE-1299): after a cancelled type-check pass that split a declaration file into pieces, the
// next full check of the same program still reports every diagnostic. A checker that finished a piece keeps that
// piece's diagnostics in its collection; reused for the same piece later, it would report nothing new.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Instant;

use tsrs_compiler::{Context, Program, ProgramOptions, new_compiler_host, new_program};
use tsrs_core::{P, Tristate};
use tsrs_vfs::{FS, vfstest};

fn program(big: &str) -> &'static Program {
    let files = [
        ("/p/a.ts", "export const a: number = 1;\n".to_string()),
        ("/p/b.ts", "export const b: string = 'b';\n".to_string()),
        ("/p/c.ts", "export const c = 3;\n".to_string()),
        ("/p/big.d.ts", big.to_string()),
    ];
    let names = files.iter().map(|(name, _)| (*name).to_string()).collect();
    let fs: Arc<dyn FS> = Arc::new(vfstest::from_map(files, true));
    let options = tsrs_core::CompilerOptions {
        no_lib: Tristate::True,
        strict: Tristate::True,
        checkers: Some(2),
        ..Default::default()
    };
    let config = tsrs_tsoptions::new_parsed_command_line(
        P::new(options),
        names,
        vec![],
        tsrs_core::tspath::ComparePathsOptions::default(),
    );
    let host = new_compiler_host("/p", fs, "", None, None, None);
    let mut opts = ProgramOptions::new(P::new(config), host);
    opts.single_threaded = Tristate::False;
    new_program(opts)
}

fn semantic(program: &'static Program, ctx: &Context) -> Vec<String> {
    program
        .get_semantic_diagnostics(ctx, None)
        .iter()
        .map(|d| format!("{}:{:?}:{}", d.file().unwrap().file_name(), d.pos(), d.code()))
        .collect()
}

#[test]
fn diagnostics_survive_a_cancelled_split_pass() {
    // SAFETY: set before any compiler code runs and before this test starts other threads (one test per process).
    unsafe { std::env::set_var("TSRS_SPLIT_FILES", "force:2") };
    let big = (0..4000).fold(String::new(), |mut s, i| {
        writeln!(s, "interface I{i} extends Missing{i} {{}}").unwrap();
        s
    });
    let live = Context::background();
    let start = Instant::now();
    let expected = semantic(program(&big), &live);
    let full = start.elapsed();
    assert_eq!(expected.len(), 4000);
    // Cancel at points spread over a whole check, three cancelled passes per program, so that some passes finish
    // pieces on other checkers before the cancellation.
    for step in 0..64u32 {
        let p = program(&big);
        let delay = full * step / 64;
        for jitter in [8u32, 10, 12] {
            let (cancelled, cancel) = live.with_cancel();
            let at = delay * jitter / 10;
            let timer = std::thread::spawn(move || {
                std::thread::sleep(at);
                cancel.call();
            });
            drop(semantic(p, &cancelled));
            timer.join().unwrap();
        }
        assert_eq!(semantic(p, &live).len(), expected.len(), "cancelled after {delay:?}");
    }
}

