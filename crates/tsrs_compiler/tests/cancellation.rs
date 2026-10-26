// EyeAuras fork: a checker of the built-in pool that saw a cancelled context is replaced on the next request, so
// the same program keeps answering correctly (Go's checkNotCanceled would panic on reuse).

use std::sync::Arc;

use tsrs_compiler::{
    Context, Program, ProgramOptions, canceled_checker_replacements, new_compiler_host, new_program,
};
use tsrs_core::{P, Tristate};
use tsrs_vfs::{FS, vfstest};

const FILES: &[(&str, &str)] = &[
    (
        "/p/a.ts",
        "export const value: number = 1;\nexport function twice(x: number) { return x * 2; }\n",
    ),
    (
        "/p/b.ts",
        "import { value, twice } from './a';\nconst s: string = twice(value);\nexport { s };\n",
    ),
    ("/p/c.ts", "import { s } from './b';\nconst n: number = s;\nexport { n };\n"),
];

fn program(regions: bool) -> &'static Program {
    let fs: Arc<dyn FS> = Arc::new(vfstest::from_map(FILES.iter().map(|&(k, v)| (k, v)), true));
    let host = new_compiler_host("/p", fs, "", None, None, None);
    let options = tsrs_core::CompilerOptions {
        no_lib: Tristate::True,
        strict: Tristate::True,
        ..Default::default()
    };
    let names = FILES.iter().map(|(name, _)| (*name).to_string()).collect();
    let config = tsrs_tsoptions::new_parsed_command_line(
        P::new(options),
        names,
        vec![],
        tsrs_core::tspath::ComparePathsOptions::default(),
    );
    let mut opts = ProgramOptions::new(P::new(config), host);
    opts.single_threaded = Tristate::True;
    opts.checker_regions = regions;
    opts.checker_diagnostic_regions = regions;
    new_program(opts)
}

fn semantic(program: &'static Program, ctx: &Context) -> Vec<String> {
    program
        .get_semantic_diagnostics(ctx, None)
        .iter()
        .map(|d| format!("{}:{:?}:{}", d.file().unwrap().file_name(), d.pos(), d.code()))
        .collect()
}

// Cancels a check of b.ts three times on one program; after each, the same program answers correctly.
fn cancel_three_times(p: &'static Program, expected: &[String]) {
    let live = Context::background();
    let file = p.get_source_file("/p/b.ts").unwrap();
    for _ in 0..3 {
        let (cancelled, cancel) = live.with_cancel();
        cancel.call();
        {
            let mut c = p.get_type_checker_for_file(&live, file);
            drop(c.get_diagnostics_exported(&cancelled, file));
            assert!(c.was_canceled());
        }
        // A whole-program request with a cancelled context returns without touching the cancelled checker.
        drop(semantic(p, &cancelled));
        assert_eq!(semantic(p, &live), expected);
        assert!(!p.get_type_checker_for_file(&live, file).was_canceled());
    }
}

#[test]
fn cancelled_checker_is_replaced_and_the_program_stays_usable() {
    let expected = semantic(program(false), &Context::background());
    assert_eq!(expected.len(), 2, "{expected:?}");

    // With and without checker regions (what an embedding host uses), each cancellation replaces the checker.
    for regions in [false, true] {
        let before = canceled_checker_replacements();
        cancel_three_times(program(regions), &expected);
        assert_eq!(canceled_checker_replacements(), before + 3);
    }
}
