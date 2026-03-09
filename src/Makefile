# Default OS detection
ifeq ($(OS),Windows_NT)
    DETECTED_OS := windows
else
    UNAME_S := $(shell uname -s)
    ifeq ($(UNAME_S),Linux)
        DETECTED_OS := linux
    endif
    ifeq ($(UNAME_S),Darwin)
        DETECTED_OS := macosx
    endif
endif

# Allow overriding OS
OS ?= $(DETECTED_OS)

# Compiler and Tools
ifeq ($(OS),windows)
    CC = x86_64-w64-mingw32-g++
    STRIP = x86_64-w64-mingw32-strip
    EXE = .exe
    OS_DEF = -D WINDOWS
else
    ifeq ($(COMP),)
        CC = clang++
    else
        CC = $(COMP)
    endif
    STRIP = strip
    EXE =
    ifeq ($(OS),macosx)
        OS_DEF = -D MACOSX
    else
        OS_DEF = -D LINUX
    endif
endif

# Architecture
ifeq ($(ARCH),)
    ifeq ($(OS),macosx)
        ARCH = arm64
    else
        ARCH = x86-64
    endif
endif

DATE = $(shell date +%y%m%d)

# Common CFLAGS
CFLAGS = -fno-exceptions -fno-rtti -Wno-parentheses

ifeq ($(OS),windows)
    CFLAGS += -D_WIN32_WINNT=0x0601
else
    CFLAGS += -fno-stack-protector -Wno-deprecated -Wno-deprecated-declarations -pthread
endif

# Arch specific CFLAGS
ifeq ($(ARCH),x86-64)
    CFLAGS += -msse4.1 -mpopcnt -D GULL_MAGIC_BITBOARDS
    VERSION = $(DATE)-$(OS)-x86_64
endif

ifeq ($(ARCH),x86-64-bmi2)
    CFLAGS += -msse4.2 -mpopcnt -mavx2 -mbmi -mbmi2 -D GULL_PDEP_BITBOARDS
    VERSION = $(DATE)-$(OS)-x86_64-bmi2
endif

ifeq ($(ARCH),arm64)
    CFLAGS += -D GULL_MAGIC_BITBOARDS
    VERSION = $(DATE)-$(OS)-arm64
endif

CFLAGS += -DVERSION=$(VERSION)

# PGO Flags
ifeq ($(OS),macosx)
    PROF_GEN = -fprofile-instr-generate
    PROF_USE = -fprofile-instr-use=default.profdata
    PROF_MERGE = xcrun llvm-profdata merge -output=default.profdata *.profraw
    NO_PEEL =
else
    PROF_GEN = -fprofile-generate
    PROF_USE = -fprofile-use
    PROF_MERGE = true
    NO_PEEL = -fno-peel-loops -fno-tracer
endif

# LDFLAGS
ifeq ($(OS),windows)
    LDFLAGS = \
        -Wl,--defsym=INFO=0x51010000 \
        -Wl,--defsym=SETTINGS=0x51000000 \
        -Wl,--defsym=SHARED=0x51020000 \
        -Wl,--defsym=DATA=0x50000000 \
        -Wl,--defsym=PAWNHASH=0x54000000 \
        -Wl,--defsym=PVHASH=0x58000000
else ifeq ($(ARCH),arm64)
    LDFLAGS =
else
    LDFLAGS = -lrt \
        -Wl,--defsym=INFO=0x300000 \
        -Wl,--defsym=SETTINGS=0x301000 \
        -Wl,--defsym=SHARED=0x302000 \
        -Wl,--defsym=DATA=0x74000000 \
        -Wl,--defsym=PAWNHASH=0x78000000 \
        -Wl,--defsym=PVHASH=0x7B000000 \
        -Wl,--defsym=HASH=0x7F000000
endif

# Targets
.PHONY: help build pgo-build debug clean

help:
	@echo 
	@echo "To build LazyGull, run:"
	@echo
	@echo "    make TARGET [OS=os] [ARCH=arch] [COMP=compiler]"
	@echo
	@echo "Where:"
	@echo
	@echo "    TARGET=build"
	@echo "        Standard build."
	@echo "    TARGET=pgo-build"
	@echo "        Profile Guided Optimization (PGO) build."
	@echo "    TARGET=debug"
	@echo "        Debug build."
	@echo
	@echo "    OS=linux | windows | macosx [DEFAULT=auto-detected]"
	@echo
	@echo "    ARCH=x86-64 | x86-64-bmi2 | arm64"
	@echo
	@echo "    COMP=g++ | clang++"
	@echo "        Use specified compiler (Linux/macOS only)"
	@echo
	@echo "To do a basic build, simply run:"
	@echo
	@echo "    make build"
	@echo

build:
	$(CC) $(CFLAGS) -O3 $(OS_DEF) -c Gull.cpp -o Gull.o
	$(CC) $(CFLAGS) -O3 $(OS_DEF) -x c++ -c tbprobe.c -o tbprobe.o
	$(CC) $(CFLAGS) Gull.o tbprobe.o $(LDFLAGS) -o LazyGull$(EXE)
	$(STRIP) LazyGull$(EXE)

pgo-build:
	rm -f *.gcda *.profraw *.profdata
	$(CC) $(CFLAGS) -O3 $(PROF_GEN) $(OS_DEF) -c Gull.cpp -o Gull.o
	$(CC) $(CFLAGS) -O3 $(PROF_GEN) $(OS_DEF) -x c++ -c tbprobe.c -o tbprobe.o
	$(CC) $(CFLAGS) $(PROF_GEN) Gull.o tbprobe.o $(LDFLAGS) -o LazyGull$(EXE)
	LLVM_PROFILE_FILE="code-%p.profraw" ./bench.sh ./LazyGull$(EXE) 11
	$(PROF_MERGE)
	$(CC) $(CFLAGS) -O3 $(PROF_USE) $(NO_PEEL) $(OS_DEF) -c Gull.cpp -o Gull.o
	$(CC) $(CFLAGS) -O3 $(PROF_USE) $(NO_PEEL) $(OS_DEF) -x c++ -c tbprobe.c -o tbprobe.o
	$(CC) $(CFLAGS) $(PROF_USE) Gull.o tbprobe.o $(LDFLAGS) -o LazyGull$(EXE)
	$(STRIP) LazyGull$(EXE)

debug:
	$(CC) $(CFLAGS) -O0 -g $(OS_DEF) -c Gull.cpp -o Gull.o
	$(CC) $(CFLAGS) -O0 -g $(OS_DEF) -x c++ -c tbprobe.c -o tbprobe.o
	$(CC) $(CFLAGS) Gull.o tbprobe.o $(LDFLAGS) -o LazyGull$(EXE)

clean:
	rm -f LazyGull LazyGull.exe Gull.o tbprobe.o
