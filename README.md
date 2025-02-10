# astd: Abseil Std Utilities for `no_std` Rust

## Overview

`astd` (Abseil-Standard Library) is a Rust library providing a selection of utility functions and data structures designed for `no_std` environments. The library takes inspiration from the Abseil C++ libraries, aiming to provide a drop in replacement for std, with entirely different underpinnings and perhaps some new features later.

The key principle behind `astd` is to offer _alternatives_ to the standard Rust library (`std`) functionalities. This is particularly relevant in `no_std` contexts where control over dependencies and resource usage is paramount. Also allows alternatives without introducing std for debugging.

I am in the process of writing a stripped down-ultra fast std replacement for bare metal from toasters to supercomputers, this project arose from that, I can get my bindings, and prove this isn't that crazy, since for this, I WILL adhere to Rust principles for safety.

## Features

Does not yet brew tea. Things will be in flux. I dropped in the starter project, std src from rust, which I'm just going to use as a template. 740 files to rewrite. I totally got this.
