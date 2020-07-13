#include "OutputDevice.h"

#include <iostream>

OutputDevice OutputDevice::Create(const std::string &name, const std::map<InputEventType, std::vector<InputEventCode>> &supportedEvents)
{
    libevdev *dev;
    libevdev_uinput *uidev;

    dev = libevdev_new();
    libevdev_set_name(dev, name.c_str());

    for (auto &entry : supportedEvents)
    {
        if (libevdev_enable_event_type(dev, as_integer(entry.first)) == 0)
        {
            for (auto &code : entry.second)
            {
                libevdev_enable_event_code(dev, as_integer(entry.first), as_integer(code), NULL);
            }
        }
    }

    libevdev_uinput_create_from_device(dev, LIBEVDEV_UINPUT_OPEN_MANAGED, &uidev);

    OutputDevice outputDevice(supportedEvents, InputDevice(dev), uidev);
    return outputDevice;
}

void OutputDevice::Write(const InputEvent &event)
{
    libevdev_uinput_write_event(this->device.get(), as_integer(event.type), as_integer(event.code), event.value);
}

std::string OutputDevice::Name()
{
    return this->inputDevice.Name();
}