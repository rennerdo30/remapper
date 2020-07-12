#include "OutputDevice.h"

OutputDevice OutputDevice::Create(const std::string &name, const std::map<InputEventType, std::vector<InputEventCode>> &supportedEvents)
{
    libevdev *dev;
    libevdev_uinput *uidev;

    dev = libevdev_new();
    libevdev_set_name(dev, name.c_str());

    for (auto &entry : supportedEvents)
    {
        libevdev_enable_event_type(dev, entry.first);

        for (auto &code : entry.second)
        {
            libevdev_enable_event_code(dev, entry.first, code, NULL);
        }
    }

    libevdev_uinput_create_from_device(dev, LIBEVDEV_UINPUT_OPEN_MANAGED, &uidev);

    OutputDevice outputDevice(dev, uidev);
}

void OutputDevice::Write(const InputEvent& event)
{
    libevdev_uinput_write_event(this->device.get(), event.type, event.code, event.value);
}