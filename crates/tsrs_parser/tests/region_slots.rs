// EyeAuras fork (POE-1299): a file parsed inside a freeable region gives its source-text slot back when the region
// is freed, whichever parse entry point copied the text into it.

use tsrs_ast::{SourceFileParseOptions, source_text_slots};
use tsrs_core::ScriptKind;
use tsrs_core::arena::Region;

#[test]
fn parsing_in_a_region_returns_the_slot_when_the_region_is_freed() {
    let parse = || {
        let region = Region::new(64 << 10);
        {
            let _scope = region.enter();
            let opts =
                || SourceFileParseOptions { file_name: "/a.json".into(), ..Default::default() };
            tsrs_parser::parse_source_file(opts(), r#"{"compilerOptions":{}}"#, ScriptKind::JSON);
            tsrs_parser::parse_source_file_owned(opts(), "let a = 1;".into(), ScriptKind::TS);
            tsrs_parser::parse_source_file_embedded(opts(), "let b = 2;", ScriptKind::TS);
        }
        drop(region);
    };
    parse();
    let before = source_text_slots();
    for _ in 0..100 {
        parse();
    }
    let after = source_text_slots();
    assert_eq!(after.high_water, before.high_water);
    assert_eq!(after.high_water - after.free, before.high_water - before.free);
}
