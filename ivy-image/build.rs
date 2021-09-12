fn main() {
    cc::Build::new()
        .file("src/stb.c")
        .warnings(false)
        .compile("stb");
}
