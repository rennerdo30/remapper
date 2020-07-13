#ifndef INPUTDEVICE_H
#define INPUTDEVICE_H

#include <memory>
#include <vector>
#include <libevdev/libevdev.h>
#include <fcntl.h>
#include <string>

#include "KeyCodes.h"

struct libevdev;

inline void destroy_libevdev(const libevdev *){};

struct InputEvent
{
    InputEvent(InputEventType type, InputEventCode code, int value) : type(type), code(code), value(value){};
    InputEvent(input_event &inputEvent) : value(inputEvent.value), code(static_cast<InputEventCode>(inputEvent.code)), type(static_cast<InputEventType>(inputEvent.type)){};

    const int value;
    const InputEventCode code;
    const InputEventType type;
};

class InputDevice
{
public:
    InputDevice(libevdev *device) : device(device, &destroy_libevdev){};
    InputDevice(const std::string &path) : path(path)
    {
        libevdev *device = libevdev_new();
        const int fd = open(path.c_str(), O_RDONLY);
        libevdev_set_fd(device, fd);

        this->device = std::shared_ptr<libevdev>(device, &destroy_libevdev);
    };

    std::string Name();
    std::string Location();
    int Vendor();
    int Version();
    int DriverVersion();
    int Bus();
    int Product();

    void Grab();
    void Ungrab();

    std::vector<InputEvent> ReadEvents();
    std::string Path() { return this->path; };

    static std::vector<InputDevice> ListDevices();

private:
    const std::string path;
    std::shared_ptr<libevdev> device;
};

#endif // INPUTDEVICE_H
