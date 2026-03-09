#ifndef PLATFORM_H
#define PLATFORM_H

#if defined(MACOSX) && !defined(LINUX)
#define LINUX
#endif

#if defined(LINUX)
#include <ctype.h>
#include <stdlib.h>
#include <setjmp.h>
#include <pthread.h>

typedef int *GHandle;
typedef int GProcess;
typedef pthread_mutex_t GMutex;
typedef pthread_cond_t GCondVar;

#define __forceinline   inline
#define __align(x)      __attribute__((aligned(x)))
#define PATH_MAX        4096
#define SIZE_T          "zu"
#define IOSIZE          4096

#elif defined(WINDOWS)
#define _POSIX_
#include <stdlib.h>
#include <setjmp.h>
#include <windows.h>
#include <intrin.h>

typedef HANDLE GMutex;
typedef HANDLE GEvent;

#define __align(x)      __attribute__((aligned(x)))
#ifndef strtok_r
#define strtok_r        strtok_s
#endif
#define SIZE_T          "Iu"
#define IOSIZE          4096
#endif

#endif // PLATFORM_H
