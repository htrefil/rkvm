#include <sys/ioctl.h>
#include <string.h>

#include "setup.h"

#define IOCTL(fd, ...) if (ioctl(fd, __VA_ARGS__) == -1) return 0;

int setup_write_fd(int fd) {
    IOCTL(fd, UI_SET_EVBIT, EV_KEY);
    IOCTL(fd, UI_SET_EVBIT, EV_SYN);
    IOCTL(fd, UI_SET_EVBIT, EV_REL);

    for (int i = 0; i < KEY_MAX; i++)
        IOCTL(fd, UI_SET_KEYBIT, i);

    IOCTL(fd, UI_SET_KEYBIT, BTN_LEFT);
    IOCTL(fd, UI_SET_KEYBIT, BTN_RIGHT);

    IOCTL(fd, UI_SET_RELBIT, REL_X);
    IOCTL(fd, UI_SET_RELBIT, REL_Y);
    IOCTL(fd, UI_SET_RELBIT, REL_WHEEL);
    
    struct uinput_setup setup;
    setup.id.bustype = BUS_USB;
    setup.id.vendor = 1;
    setup.id.product = 1;
    setup.ff_effects_max = 0;
    strcpy(setup.name, "rkvm");

    IOCTL(fd, UI_DEV_SETUP, &setup);
    IOCTL(fd, UI_DEV_CREATE);

    return 1;
}

int destroy_write_fd(int fd) {
    IOCTL(fd, UI_DEV_DESTROY);
    return 1;
}

int setup_read_fd(int fd) {
    IOCTL(fd, EVIOCGRAB, 1);
    return 1;
}
