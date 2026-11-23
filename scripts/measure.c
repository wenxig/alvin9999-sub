#include <stdio.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

int main(int argc, char *argv[]) {
    if (argc != 2) {
        return 2;
    }

    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);
    pid_t child = fork();
    if (child == 0) {
        execl(argv[1], argv[1], (char *)NULL);
        perror(argv[1]);
        _exit(127);
    }

    int status;
    struct rusage usage;
    if (child < 0 || wait4(child, &status, 0, &usage) < 0) {
        perror("measure");
        return 1;
    }
    clock_gettime(CLOCK_MONOTONIC, &end);
    if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        return 1;
    }

    double wall = (end.tv_sec - start.tv_sec) * 1000.0
        + (end.tv_nsec - start.tv_nsec) / 1000000.0;
    double cpu = (usage.ru_utime.tv_sec + usage.ru_stime.tv_sec) * 1000.0
        + (usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) / 1000.0;
    printf("%.3f %.3f %ld\n", wall, cpu, usage.ru_maxrss);
    return 0;
}
