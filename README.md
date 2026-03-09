The LazyGull Chess Engine
=========================

**NOTICE:** LazyGull is no longer maintained.

LazyGull is a free UCI chess engine derived from Gull 3.  Compared to Gull,
LazyGull implements the following new features:

* Syzygy Tablebase support;
* LazySMP implementation;
* PDEP Bitboards for modern CPUs;
* Bug fixes; and
* Portable to Windows, Linux and MacOSX.

Recent Maintenance (Cleanup2026 branch)
---------------------------------------

The `Cleanup2026` branch introduces significant structural and code quality improvements:

* **Unified Build System**: Replaced platform-specific Makefiles with a single, cross-platform `Makefile` featuring automatic OS detection.
* **Modern C++**: Modernized the codebase to the C++11 standard, utilizing `nullptr`, `std::chrono` for high-resolution timing, and type-safe templates.
* **Structural Improvements**: Organized platform-specific code into a dedicated `platform/` directory and standardized file extensions (e.g., `data.cpp`).
* **Enhanced Stability**: Improved the robustness of PV extraction and move legality checking, resolving issues with illegal moves appearing in the Principal Variation.
* **Simplified Repository**: Tablebase probing code is now fully integrated, removing external download dependencies during the build process.

To build, simply run the following command in the `src` directory:

    make build

The build system will automatically detect your operating system (Linux, Windows, or MacOSX).

License
-------

The original Gull 3 source code is "public domain".  New modifications are
released under the MIT License.
