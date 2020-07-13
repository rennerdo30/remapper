#ifndef OUTPUTDEVICE_H
#define OUTPUTDEVICE_H

#include <memory>
#include "libevdev-1.0/libevdev/libevdev.h"
#include "libevdev-1.0/libevdev/libevdev-uinput.h"
#include "KeyCodes.h"
#include <map>
#include <vector>

#include "InputDevice.h"

struct libevdev_uinput;
inline void destroy_libevdev_uinput(libevdev_uinput *uidev){/*libevdev_uinput_destroy(uidev);*/};

class OutputDevice
{
public:
    void Write(const InputEvent &event);

    static OutputDevice Create(const std::string &name, const std::map<InputEventType, std::vector<InputEventCode>> &supportedEvents);

    InputDevice Device() { return this->inputDevice; };
    std::string Name();
    std::map<InputEventType, std::vector<InputEventCode>> SupportedEvents() { return this->supportedEvents; };

private:
    explicit OutputDevice(const std::map<InputEventType, std::vector<InputEventCode>> &supportedEvents, const InputDevice &inputDevice, libevdev_uinput *device) : supportedEvents(supportedEvents), inputDevice(std::move(inputDevice)), device(device, &destroy_libevdev_uinput){};

    const std::map<InputEventType, std::vector<InputEventCode>> supportedEvents;

    std::shared_ptr<libevdev_uinput> device;
    InputDevice inputDevice;
};

#endif