# cuse-lowlevel

[crates.io]: https://crates.io/crates/cuse-lowlevel

**Raw bindings to the low level api of cuse and fuse in libfuse3**

---

## About

This crate is heavily based on libfuse-sys by Richard Wiedenhöft. See the [original repository](https://github.com/richard-w/libfuse-sys)

This fork here contains only the relevant subset of code of libfuse-sys to access the low-level api of cuse.

## Using cuse-lowlevel

Add the dependencies to your Cargo.toml
```toml
[dependencies]
cuse-lowlevel = { version = "0.1"}
libc = "*"
```

## License

This crate itself is published under the MIT license while libfuse is published under
LGPL2+. Take special care to ensure the terms of the LGPL2+ are honored when using this
crate.