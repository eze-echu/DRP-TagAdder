# DRP tag adder

A Little rust program to add the DRP tag to all instances in a region

## Usage

```shell
TagAllInstances --profile <AWS_PROFILE> --drp-tier [Gold|Silver|Bronze(Default)]
```

## Build
```shell
RUSTFLAGS="-Z threads=16" cargo +nightly build -Zprofile-hint-mostly-unused --release
```