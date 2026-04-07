use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};

fn main() {
    let prebuilt = Prebuilt::fetch(Source::LATEST, "target/ovmf").unwrap();
    println!("{}", prebuilt.get_file(Arch::X64, FileType::Code).display());
    println!("{}", prebuilt.get_file(Arch::X64, FileType::Vars).display());
}
