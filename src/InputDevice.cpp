#include "InputDevice.h"

#include <string>
#include <glob.h>
#include <cstring>
#include <sstream>

#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>

std::vector<std::string> glob(const std::string &pattern);

std::vector<InputDevice> InputDevice::ListDevices()
{
    std::vector<InputDevice> devices;
    for (const auto &entry : glob("/dev/input/event*"))
    {
        libevdev *device = libevdev_new();
        const int fd = open(entry.c_str(), O_RDONLY);
        libevdev_set_fd(device, fd);

        InputDevice inputDevice(device);
        devices.push_back(device);
    }

    return devices;
}

std::string InputDevice::Name()
{
    std::string name = libevdev_get_name(this->device.get());
    return name;
}

std::string InputDevice::Location()
{
    std::string location = libevdev_get_phys(this->device.get());
    return location;
}

int InputDevice::Vendor()
{
    int vendor = libevdev_get_id_vendor(this->device.get());
    return vendor;
}

int InputDevice::Product()
{
    int product = libevdev_get_id_product(this->device.get());
    return product;
}

int InputDevice::Version()
{
    int version = libevdev_get_id_version(this->device.get());
    return version;
}

int InputDevice::DriverVersion()
{
    int driverVersion = libevdev_get_driver_version(this->device.get());
    return driverVersion;
}

int InputDevice::Bus()
{
    int bus = libevdev_get_id_bustype(this->device.get());
    return bus;
}

void InputDevice::Grab()
{
    libevdev_grab(this->device.get(), LIBEVDEV_GRAB);
}

void InputDevice::Ungrab()
{
    libevdev_grab(this->device.get(), LIBEVDEV_UNGRAB);
}

std::vector<InputEvent> InputDevice::ReadEvents()
{
    std::vector<InputEvent> events;
    while (libevdev_has_event_pending(this->device.get()) == 1)
    {
        input_event event;
        if (libevdev_next_event(this->device.get(), LIBEVDEV_READ_FLAG_NORMAL, &event) == LIBEVDEV_READ_STATUS_SUCCESS)
        {
            events.emplace_back(event);
        }
    }

    return events;
}

void InputDevice::from_toml(const toml::value &v)
{
    libevdev *device = libevdev_new();
    const int fd = open(toml::find<std::string>(v, "path").c_str(), O_RDONLY);
    libevdev_set_fd(device, fd);
    this->device = std::shared_ptr<libevdev>(device, &destroy_libevdev);
}


std::vector<std::string> glob(const std::string &pattern)
{
    using namespace std;

    // glob struct resides on the stack
    glob_t glob_result;
    memset(&glob_result, 0, sizeof(glob_result));

    // do the glob operation
    int return_value = glob(pattern.c_str(), GLOB_TILDE, NULL, &glob_result);
    if (return_value != 0)
    {
        globfree(&glob_result);
        stringstream ss;
        ss << "glob() failed with return_value " << return_value << endl;
        throw std::runtime_error(ss.str());
    }

    // collect all the filenames into a std::list<std::string>
    vector<string> filenames;
    for (size_t i = 0; i < glob_result.gl_pathc; ++i)
    {
        filenames.push_back(string(glob_result.gl_pathv[i]));
    }

    // cleanup
    globfree(&glob_result);

    // done
    return filenames;
}
