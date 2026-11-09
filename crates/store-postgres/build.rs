fn main() {
    // Rebuild when migrations change so the embedded migrator stays current.
    println!("cargo:rerun-if-changed=migrations");
}
