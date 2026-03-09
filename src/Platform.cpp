#include <assert.h>
#include <errno.h>
#include <limits.h>
#include <math.h>
#include <setjmp.h>
#include <signal.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifdef LINUX
#ifndef __aarch64__
#include <xmmintrin.h>
#include <popcntintrin.h>
#include <x86intrin.h>
#endif
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/wait.h>
#ifdef MACOSX
#include <mach/mach_time.h>
#include <mach-o/dyld.h>
#define MAP_ANONYMOUS   MAP_ANON
#else
#include <sys/prctl.h>
#endif
#endif

#ifdef WINDOWS
#define _POSIX_
#include <windows.h>
#include <xmmintrin.h>
#include <popcntintrin.h>
#include <x86intrin.h>
#endif

#ifndef UINT64_MAX
#define UINT64_MAX  0xFFFFFFFFFFFFFFFFull
#endif
#ifndef UINT32_MAX
#define UINT32_MAX  0xFFFFFFFF
#endif
#ifndef UINT8_MAX
#define UINT8_MAX   0xFF
#endif

#ifndef __aarch64__
#define builtin_cpuid(f, ax, bx, cx, dx)    \
    __asm__ __volatile__ ("cpuid" : "=a" (ax), "=b" (bx), "=c" (cx), \
        "=d" (dx) : "a" (f))
#endif

#ifdef PAGE_SIZE
#undef PAGE_SIZE
#endif
#define PAGE_SIZE       4096
#define SIZE(size)      ((((size)-1) / PAGE_SIZE) * PAGE_SIZE + PAGE_SIZE)

/*
 * Log a message.
 */
static void log(const char *format, ...)
{
    FILE *stream = fopen("lazygull.log", "a");
    if (stream == NULL)
        return;
    va_list ap;
    va_start(ap, format);
    vfprintf(stream, format, ap);
    va_end(ap);
    fclose(stream);
}

/*
 * Print an error message.
 */
#define error(format, ...)                                              \
    do {                                                                \
        log("error: " format "\n", ##__VA_ARGS__);                      \
        fprintf(stderr, "error: " format "\n", ##__VA_ARGS__);          \
        abort();                                                        \
    } while (false)

/*
 * Init an object name.
 */
void init_object_name(char *name, size_t len, const char *basename,
    unsigned id, int idx)
{
#ifdef WINDOWS
    int r = snprintf(name, len, "Local\\LazyGull_%u_%s_%d", id, basename, idx);
    if (r < 0 || r >= len)
        error("failed to create object name (%d)", GetLastError());
#else
    int r = snprintf(name, len, "/LazyGull_%u_%s_%d", id, basename, idx);
    if (r < 0 || r >= len)
        error("failed to create object name: %s", strerror(errno));
#endif
}

#ifdef WINDOWS
typedef struct
{
    char name[256];
    HANDLE handle;
} GHandleInfo;

static GHandleInfo handleInfo[16] = {0};
#endif

/*
 * Init an object.
 */
void *init_object(const char *object, size_t size, void *addr,
    bool create, bool readonly, bool map, const void *value)
{
#ifdef WINDOWS
    size_t size2 = SIZE(size);
    HANDLE handle = INVALID_HANDLE_VALUE;
    if (object != NULL)
    {
        if (create)
        {
            handle = CreateFileMapping(INVALID_HANDLE_VALUE, NULL, 
                PAGE_READWRITE, (DWORD)(size2 >> 32),
                (DWORD)(size2 & 0xFFFFFFFF), object);
            for (unsigned i = 0;
                    i < sizeof(handleInfo) / sizeof(handleInfo[0]); i++)
            {
                if (handleInfo[i].name[0] == '\0')
                {
                    strncpy(handleInfo[i].name, object,
                        sizeof(handleInfo[i].name)-1);
                    handleInfo[i].handle = handle;
                    break;
                }
            }
        }
        else
            handle = OpenFileMapping(FILE_MAP_ALL_ACCESS, FALSE, object);
    }
    else
        handle = CreateFileMapping(INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE,
            (DWORD)(size2 >> 32), (DWORD)(size2 & 0xFFFFFFFF), NULL);
    if ((handle == INVALID_HANDLE_VALUE || handle == NULL) &&
            (create || GetLastError() != ERROR_ALREADY_EXISTS))
        error("failed to open file mapping \"%s\" (%d)", object,
            GetLastError());
    void *ptr = NULL;
    if (map)
    {
        DWORD access = FILE_MAP_READ |
            (readonly && value == NULL? 0: FILE_MAP_WRITE);
        ptr = MapViewOfFileEx(handle, access, 0, 0, size2, addr);
        if (ptr == NULL)
            error("failed to map file mapping \"%s\" (%d)", object,
                GetLastError());
        if (value != NULL)
        {
            memcpy(ptr, value, size);
            DWORD old_prot;
            if (readonly &&
                    !VirtualProtect(ptr, size2, PAGE_READONLY, &old_prot))
                error("failed to protect object \"%s\" (%d)", object,
                    GetLastError());
        }
    }
    if (!create || object == NULL)
        CloseHandle(handle);
    return ptr;
#else
    int fd = -1;
    int flags = 0;
    if (object != NULL)
    {
        if (create)
            fd = shm_open(object, O_RDWR | O_CREAT | O_CLOEXEC,
                S_IRUSR | S_IWUSR);
        else
            fd = shm_open(object, O_RDWR | O_CLOEXEC, 0);
        if (fd < 0)
            error("failed to open object %s: %s", object, strerror(errno));
        if (create && ftruncate(fd, SIZE(size)) != 0)
            error("failed to truncate object %s: %s", object, strerror(errno));
        flags |= MAP_SHARED;
    }
    else
        flags |= MAP_PRIVATE | MAP_ANONYMOUS;
    if (map)
    {
        int prot = PROT_READ | (readonly && value == NULL? 0: PROT_WRITE);
        flags |= (addr == NULL? 0: MAP_FIXED);
        void *ptr = mmap(addr, SIZE(size), prot, flags, fd, 0);
        if (ptr == MAP_FAILED || (addr != NULL && ptr != addr))
            error("failed to map object %s: %s", object, strerror(errno));
        if (value != NULL)
        {
            memcpy(ptr, value, size);
            if (readonly && mprotect(ptr, SIZE(size), PROT_READ) != 0)
                error("failed to protect object %s: %s", object,
                    strerror(errno));
        }
        if (fd > 0)
            close(fd);
        return ptr;
    }
    if (fd > 0)
        close(fd);
    return NULL;
#endif
}

/*
 * Remove object.
 */
void remove_object(const char *object)
{
#ifdef WINDOWS
    for (unsigned i = 0; i < sizeof(handleInfo) / sizeof(handleInfo[0]); i++)
    {
        if (strcmp(handleInfo[i].name, object) == 0)
        {
            handleInfo[i].name[0] = '\0';
            CloseHandle(handleInfo[i].handle);
            handleInfo[i].handle = NULL;
            return;
        }
    }
    error("failed to remove object \"%s\"", object);
#else
    if (shm_unlink(object) != 0)
        error("failed to unlink object %s: %s", object, strerror(errno));
#endif
}

/*
 * Delete an object.
 */
void delete_object(void *addr, size_t size)
{
#ifdef WINDOWS
    if (!UnmapViewOfFile(addr))
        error("failed to unmap object (%d)", GetLastError());
#else
    if (munmap(addr, SIZE(size)) != 0)
        error("failed to unmap object: %s", strerror(errno));
#endif
}

/*
 * Create a child process.
 */
void create_child(const char *hashName, const char *pvHashName,
    const char *pawnHashName, const char *dataName, const char *settingsName,
    const char *sharedName, const char *infoName, const char *tbPath)
{
#ifdef WINDOWS
    char name[PATH_MAX];
    char command[10 * PATH_MAX];
    PROCESS_INFORMATION procInfo;
    STARTUPINFO startInfo;
      
    memset(&procInfo, 0, sizeof(procInfo));
    memset(&startInfo, 0, sizeof(startInfo));
  
    startInfo.cb = sizeof(STARTUPINFO);
    startInfo.dwFlags |= STARTF_USESTDHANDLES;
    startInfo.hStdError = GetStdHandle(STD_ERROR_HANDLE); 
    startInfo.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE); 
    startInfo.hStdInput = INVALID_HANDLE_VALUE;
  
    if (GetModuleFileName(NULL, name, sizeof(name)-1) >= sizeof(name)-1)
        error("failed to get module name");
    int len = snprintf(command, sizeof(command)-1,
        "\"%s\" child \"%s\" \"%s\" \"%s\" \"%s\" \"%s\" \"%s\" \"%s\" \"%s\"",
        name, hashName, pvHashName, pawnHashName, dataName, settingsName,
        sharedName, infoName, tbPath);
    if (len < 0 || len >= sizeof(command)-1)
        error("failed to create command line for child"); 

    BOOL success = CreateProcess(NULL, command, NULL, NULL, TRUE,
        0, NULL, NULL, &startInfo, &procInfo);
    if (!success)
        error("failed to create child process (%d)", GetLastError());
    CloseHandle(procInfo.hThread);
#else
    pid_t pid = fork();
    if (pid < 0)
        error("failed to fork: %s", strerror(errno));
    if (pid != 0)
        return;
#ifndef MACOSX
    prctl(PR_SET_PDEATHSIG, SIGHUP);
#endif
    char exe[PATH_MAX];
#ifndef MACOSX
    ssize_t len = readlink("/proc/self/exe", exe, sizeof(exe)-1);
    if (len < 0)
        error("failed to read link: %s", strerror(errno));
    exe[len] = '\0';
#else
    uint32_t size = sizeof(exe);
    if (_NSGetExecutablePath(exe, &size) != 0)
        error("failed to get executable path");
#endif
    execl(exe, "Gull", "child", hashName, pvHashName, pawnHashName,
        dataName, settingsName, sharedName, infoName, tbPath, NULL);
    error("failed to exec: %s", strerror(errno));
#endif
}

/*
 * Get the process ID.
 */
unsigned get_pid(void)
{
#ifdef WINDOWS
    return (unsigned)GetCurrentProcessId();
#else
    return (unsigned)getpid();
#endif
}

/*
 * Get the number of CPUs.
 */
unsigned get_num_cpus(void)
{
#ifdef WINDOWS
    SYSTEM_INFO sysinfo;
    GetSystemInfo(&sysinfo);
    return (unsigned)sysinfo.dwNumberOfProcessors;
#else
    return (unsigned)sysconf(_SC_NPROCESSORS_ONLN);
#endif
}

/*
 * Nuke a child process.
 */
static void nuke_child(unsigned pid)
{
#ifdef WINDOWS
    HANDLE handle = OpenProcess(PROCESS_ALL_ACCESS, FALSE, (DWORD)pid);
    if (handle == NULL)
        return;
    TerminateProcess(handle, EXIT_SUCCESS);
    if (WaitForSingleObject(handle, INFINITE) != WAIT_OBJECT_0)
        error("failed to terminate child (%d)", GetLastError());
    CloseHandle(handle);
#else
    kill((pid_t)pid, SIGKILL);
    waitpid(pid, NULL, 0);
#endif
}

/*
 * Sleep for `ms' milliseconds.
 */
static void msleep(unsigned ms)
{
#ifdef WINDOWS
    Sleep(ms);
#else
    usleep(1000 * ms);
#endif
}

/*
 * Get the time in milliseconds.
 */
int64_t get_time()
{
#ifdef WINDOWS
    return GetTickCount64();
#else
#ifndef MACOSX
    // Linux:
    struct timespec ts;
    unsigned tick = 0;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    tick  = ts.tv_nsec / 1000000;
    tick += ts.tv_sec * 1000;
    return tick;
#else
    // MacOSX:
    static mach_timebase_info_data_t info;
    static bool init = false;
    if (!init)
    {
        mach_timebase_info(&info);
        init = true;
    }
    return (int64_t)((mach_absolute_time() * info.numer / info.denom) / 1000000);
#endif
#endif
}

/*
 * Threads.
 */
#ifdef WINDOWS
static void mutex_init(GMutex *mutex)
{
    SECURITY_ATTRIBUTES attr;
    memset(&attr, 0, sizeof(attr));
    attr.nLength = sizeof(attr);
    attr.bInheritHandle = TRUE;
    *mutex = CreateMutex(&attr, FALSE, NULL);
    if (*mutex == NULL)
        error("failed to create mutex (%d)", GetLastError());
}

static void mutex_lock(GMutex *mutex)
{
    if (WaitForSingleObject(*mutex, INFINITE) != WAIT_OBJECT_0)
        error("failed to lock mutex (%d)", GetLastError());
}

static bool mutex_lock(GMutex *mutex, uint64_t timeout)
{
    switch (WaitForSingleObject(*mutex, (DWORD)timeout))
    {
        case WAIT_OBJECT_0:
            return false;
        case WAIT_TIMEOUT:
            return true;
        default:
            error("failed to lock mutex (%d)", GetLastError());
    }
}

static void mutex_unlock(GMutex *mutex)
{
    if (!ReleaseMutex(*mutex))
        error("failed to unlock mutex (%d)", GetLastError());
}

static void mutex_free(GMutex *mutex)
{
    CloseHandle(*mutex);
}

static void event_init(GEvent *event)
{
    SECURITY_ATTRIBUTES attr;
    memset(&attr, 0, sizeof(attr));
    attr.nLength = sizeof(attr);
    attr.bInheritHandle = TRUE;
    *event = CreateEvent(&attr, TRUE, FALSE, NULL);
    if (*event == NULL)
        error("failed to create event (%d)", GetLastError());
}

static void event_signal(GEvent *event)
{
    SetEvent(*event);
    ResetEvent(*event);
}

static void event_wait(GEvent *event, GMutex *mutex)
{
    if (SignalObjectAndWait(*mutex, *event, INFINITE, FALSE) !=
            WAIT_OBJECT_0)
        error("failed to wait for event (%d)", GetLastError());
    mutex_lock(mutex);
}

static void event_free(GEvent *event)
{
    CloseHandle(*event);
}
#else
static void mutex_init(GMutex *mutex)
{
    pthread_mutexattr_t attrs;
    pthread_mutexattr_init(&attrs);
    pthread_mutexattr_setpshared(&attrs, PTHREAD_PROCESS_SHARED);
    pthread_mutex_init(mutex, &attrs);
}

static void cond_init(GCondVar *condVar)
{
    pthread_condattr_t attrs;
    pthread_condattr_init(&attrs);
    pthread_condattr_setpshared(&attrs, PTHREAD_PROCESS_SHARED);
#ifndef MACOSX
    pthread_condattr_setclock(&attrs, CLOCK_MONOTONIC);
#endif
    pthread_cond_init(condVar, &attrs);
}

#define mutex_unlock    pthread_mutex_unlock

static void mutex_lock(GMutex *mutex)
{
    pthread_mutex_lock(mutex);
}

static bool mutex_lock(GMutex *mutex, uint64_t timeout)
{
#ifndef MACOSX
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts) != 0)
    {
        // Backup strat.
        mutex_lock(mutex);
        return false;
    }
    ts.tv_nsec += timeout * 1000000;
    int r = pthread_mutex_timedlock(mutex, &ts);
    return (r == ETIMEDOUT);
#else
    // MacOS doesn't support timedlock.
    mutex_lock(mutex);
    return false;
#endif
}

static void mutex_free(GMutex *mutex)
{
    // NOP 
}

#define cond_signal     pthread_cond_signal
#define cond_broadcast  pthread_cond_broadcast
#define cond_wait       pthread_cond_wait

static void cond_free(GCondVar *mutex)
{
    // NOP
}
#endif

/*
 * Input.
 */
#ifdef WINDOWS
static DWORD forward(LPVOID param)
{
    char buf[4 * IOSIZE];
    HANDLE in  = GetStdHandle(STD_INPUT_HANDLE);
    HANDLE out = (HANDLE)param;
    while (true)
    {
        DWORD len;
        if (!ReadFile(in, buf, sizeof(buf), &len, NULL))
            error("failed to read input (%d)", GetLastError());
        if (len == 0)
        {
            CloseHandle(out);
            return 0;
        }

        DWORD ptr = 0;
        while (ptr < len)
        {
            DWORD writelen;
            if (!WriteFile(out, buf + ptr, len - ptr, &writelen, NULL))
                error("failed to forward input (%d)", GetLastError());
            ptr += writelen;
        }

        FlushFileBuffers(out);
    }
}
#endif

static bool get_line(char *line, unsigned linelen, uint64_t timeout)
{
    static char buf[4 * IOSIZE];
    static unsigned ptr = 0, end = 0;
    unsigned i = 0;

#ifdef WINDOWS
    static HANDLE handle = INVALID_HANDLE_VALUE;
    static GEvent event = NULL;
    static bool init = false;
    if (!init)
    {
        handle = GetStdHandle(STD_INPUT_HANDLE);
        if (GetFileType(handle) == FILE_TYPE_PIPE)
        {
            char name[256];
            int res = snprintf(name, sizeof(name)-1,
                "\\\\.\\pipe\\LazyGull_%u_pipe", get_pid());
            if (res < 0 || res >= sizeof(name)-1)
                error("failed to create pipe name");
            HANDLE out = CreateNamedPipe(name,
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, 2,
                4 * IOSIZE, 4 * IOSIZE, 0, NULL);
            if (out == INVALID_HANDLE_VALUE)
                error("failed to create named pipe #1 (%d)", GetLastError());
            handle = CreateFile(name, GENERIC_READ, 0, NULL,
                OPEN_EXISTING, FILE_FLAG_OVERLAPPED, NULL);
            if (handle == INVALID_HANDLE_VALUE)
                error("failed to create named pipe #2 (%d)", GetLastError());
            HANDLE thread = CreateThread(NULL, 0, forward, (LPVOID)out, 0,
                NULL);
            if (thread == NULL)
                error("failed to create thread (%d)", GetLastError());
        }
        event_init(&event);
        init = true;
    }
#endif

    while (true)
    {
        bool space = false;
        while (ptr < end)
        {
            if (i >= linelen)
                error("input buffer overflow");
            char c = buf[ptr++];
            switch (c)
            {
                case ' ': case '\r': case '\t': case '\n':
                    if (!space)
                    {
                        line[i++] = ' ';
                        space = true;
                    }
                    if (c == '\n')
                    {
                        line[i-1] = '\0';
                        return false;
                    }
                    continue;
                default:
                    space = false;
                    line[i++] = c;
                    continue;
            }
        }

#ifdef WINDOWS
        OVERLAPPED overlapped;
        memset(&overlapped, 0, sizeof(overlapped));
        overlapped.hEvent = event;
        DWORD len;
        if (!ReadFile(handle, buf, sizeof(buf), &len, &overlapped))
        {
            if (GetLastError() != ERROR_IO_PENDING)
                error("failed to read input (%d)", GetLastError());
            bool timedout = false;

            switch (WaitForSingleObject(event, (DWORD)timeout))
            {
                case WAIT_TIMEOUT:
                    if (!CancelIo(handle))
                        error("failed to cancel input (%d)", GetLastError());
                    timedout = true;
                    break;
                case WAIT_OBJECT_0:
                    break;
                default:
                    error("failed to wait for input (%d)", GetLastError());
            }
            if (!GetOverlappedResult(handle, &overlapped, &len, FALSE))
            {
                if (timedout && GetLastError() == ERROR_OPERATION_ABORTED)
                    return true;
                error("failed to get input result (%d)", GetLastError());
            }
        }

        if (len == 0)
        {
            line[0] = EOF;
            return false;
        }

        ptr = 0;
        end = len;
#else
        struct timeval tv;
        uint64_t cap_timeout = timeout;
        if (cap_timeout > 1000000000ull) cap_timeout = 1000000000ull;
        tv.tv_sec  = cap_timeout / 1000;
        tv.tv_usec = (cap_timeout % 1000) * 1000;
        fd_set fds;
        FD_ZERO(&fds);
        FD_SET(STDIN_FILENO, &fds);
        ssize_t res = select(STDIN_FILENO+1, &fds, NULL, NULL, &tv);
        if (res < 0)
            error("failed to wait for input: %s", strerror(errno));
        if (res == 0)
            return true;   // Timeout

        do
        {
            res = read(STDIN_FILENO, buf, sizeof(buf));
        }
        while (res < 0 && errno == EINTR);

        if (res == 0)
        {
            line[0] = (char)EOF;
            return false;
        }
        if (res < 0)
            error("failed to read input: %s", strerror(errno));

        ptr = 0;
        end = res;
#endif
    }
}

/*
 * Output.
 */
static void put_line(char *line, unsigned linelen)
{
#ifdef WINDOWS
    if (linelen > _POSIX_PIPE_BUF)
#else
    if (linelen > PIPE_BUF)
#endif
    {
        log("warning: output \"%s\" too long (max is %u, got %u)\n", line,
#ifdef WINDOWS
            _POSIX_PIPE_BUF, 
#else
            PIPE_BUF,
#endif
            linelen);
        return;
    }

#ifdef WINDOWS
    HANDLE handle = GetStdHandle(STD_OUTPUT_HANDLE);
    DWORD len;
    if (!WriteFile(handle, line, linelen, &len, NULL) || len != linelen)
        error("failed to write output (%d)", GetLastError());
    FlushFileBuffers(handle);
#else
    int res;
    do
    {
        res = write(STDOUT_FILENO, line, linelen);
    }
    while (res < 0 && (errno == EINTR || errno == EAGAIN));
    if (res != linelen)
        error("failed to write output: %s", strerror(errno));
#endif
}

/*
 * O/S specific init.
 */
static void init_os(void)
{
#ifdef WINDOWS
    HANDLE handle = GetStdHandle(STD_INPUT_HANDLE);
    DWORD mode;
    if (GetConsoleMode(handle, &mode))
    {
        mode &= ~ENABLE_MOUSE_INPUT;
        mode &= ~ENABLE_WINDOW_INPUT;
        mode |= ENABLE_LINE_INPUT;
        mode |= ENABLE_ECHO_INPUT;
        SetConsoleMode(handle, mode);
        FlushConsoleInputBuffer(handle);
    }

    HANDLE job = CreateJobObject(NULL, NULL);
    if (job == NULL)
        error("failed to create job object (%d)", GetLastError());
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION info;
    memset(&info, 0, sizeof(info));
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info,
                sizeof(info));
    AssignProcessToJobObject(job, GetCurrentProcess());     // Allowed to fail
#else
    int fds[2];
#ifndef MACOSX
    if (pipe2(fds, O_CLOEXEC) != 0)
        return;
#else
    if (pipe(fds) != 0)
        return;
    fcntl(fds[0], F_SETFD, FD_CLOEXEC);
    fcntl(fds[1], F_SETFD, FD_CLOEXEC);
#endif
    pid_t pid = getpid();
    if (fork() == 0)
    {
#ifndef MACOSX
        prctl(PR_SET_PDEATHSIG, SIGHUP);
#endif
        close(fds[1]);
        char c;
        int r = read(fds[0], &c, sizeof(c));
        kill(-pid, SIGKILL);
        _exit(0);
    }
    close(fds[0]);
#endif
}
