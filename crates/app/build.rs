// Let the binary find `libcef.so` in its own folder (the layout the Arch
// package installs). The developer rpath to `CEF_PATH` comes from the
// local `.cargo/config.toml`, and is searched after this one.
fn main() {
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
}
