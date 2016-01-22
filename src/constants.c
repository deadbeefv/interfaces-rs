#include <stdlib.h>
#include <stdint.h>

#include <sys/ioctl.h>
#include <sys/socket.h>
#include <net/if.h>

typedef struct constant {
    const char* name;
    uint64_t    value;
} constant_t;

constant_t* rust_get_constants() {
    static constant_t constants[] = {
        
            { "SIOCGIFFLAGS", SIOCGIFFLAGS },
        
            { "SIOCSIFFLAGS", SIOCSIFFLAGS },
        

        // End of list sentinel
        { NULL, 0 },
    };

    return constants;
}
