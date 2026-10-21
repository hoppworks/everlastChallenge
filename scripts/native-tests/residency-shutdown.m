// Exercise the public residency collection lifecycle, without a GPU workload.
#import <Foundation/Foundation.h>
#include "ggml-metal-device.h"
#include <stdio.h>
#include <time.h>
#include <unistd.h>

static double milliseconds(void) {
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return now.tv_sec * 1000.0 + now.tv_nsec / 1000000.0;
}

int main(void) {
    @autoreleasepool {
        for (int iteration = 0; iteration < 3; ++iteration) {
            ggml_metal_rsets_t sets = ggml_metal_rsets_init();
            // Let the real heartbeat enter its 500ms wait.
            usleep(20000);
            double start = milliseconds();
            ggml_metal_rsets_free(sets);
            double elapsed = milliseconds() - start;
            printf("{\"iteration\":%d,\"shutdownMs\":%.3f}\n", iteration, elapsed);
            if (elapsed >= 150.0) {
                fprintf(stderr, "residency shutdown waited for its heartbeat timeout\n");
                return 1;
            }
        }
    }
    return 0;
}
