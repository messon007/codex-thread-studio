//! Reproduce the embedded grammar notices from the pinned upstream data.
fn main() {
    let mut markdown = String::from("# Embedded syntax acknowledgements\n\nGenerated from the pinned two-face syntax bundle using its acknowledgement API.\nLicense texts and paths below come from the unmodified upstream asset.\n\n");
    for license in two_face::acknowledgement::listing().for_syntaxes() {
        license.write_md(&mut markdown);
    }
    print!("{markdown}");
}
