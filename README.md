[![Coverage Status](https://coveralls.io/repos/github/OSSystems/find-binary-version-rs/badge.svg?branch=master)](https://coveralls.io/github/OSSystems/find-binary-version-rs?branch=master)
[![Documentation](https://docs.rs/find-binary-version/badge.svg)](https://docs.rs/find-binary-version)

# find-binary-version

The library provide a way for reading version from the binaries files

| Platform | Build Status |
| -------- | ------------ |
| Linux | [![build status](https://github.com/OSSystems/find-binary-version-rs/workflows/CI%20(Linux)/badge.svg)](https://github.com/OSSystems/find-binary-version-rs/actions) |
| macOS | [![build status](https://github.com/OSSystems/find-binary-version-rs/workflows/CI%20(macOS)/badge.svg)](https://github.com/OSSystems/find-binary-version-rs/actions) |
| Windows | [![build status](https://github.com/OSSystems/find-binary-version-rs/workflows/CI%20(Windows)/badge.svg)](https://github.com/OSSystems/find-binary-version-rs/actions) |

---

### Dependencies

You must have `libarchive` properly installed on your system in order to use
this. If building on *nix systems, `pkg-config` is used to locate the
`libarchive`; on Windows `vcpkg` will be used to locating the `libarchive`.

The minimum supported Rust version is 1.44.

### Features

The following know patterns are supported allowing the version to be detected
without the need for any user specification:

* U-Boot
* LinuxKernel

Other formats are supported through the `version_with_pattern` function,
which will look for a given regular expression on the given binary.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
