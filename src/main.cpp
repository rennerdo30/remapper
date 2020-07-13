#include <iostream>

#include <toml.hpp>
#include <boost/program_options.hpp>

#include "InputDevice.h"
#include "InputMap.h"
#include "KeyCodes.h"

#include <filesystem>

namespace po = boost::program_options;

inline bool exists(const std::string &name)
{
    if (FILE *file = fopen(name.c_str(), "r"))
    {
        fclose(file);
        return true;
    }
    else
    {
        return false;
    }
}

std::map<InputEventType, std::vector<InputEventCode>> CodeMapToCapabilities(const std::map<InputEventCode, InputEventCode> &codeMap)
{
    std::map<InputEventType, std::vector<InputEventCode>> result;

    std::vector<InputEventCode> codes;
    for (auto &entry : codeMap)
    {
        codes.push_back(entry.second);
    }

    result[InputEventType::ABS] = codes;
    result[InputEventType::KEY] = codes;

    return result;
}

int main(int argc, char **argv)
{
    if (!exists("config.toml"))
    {
        std::ofstream output("config.toml");
        output.close();
    }

    auto data = toml::parse("config.toml");

    po::options_description desc("Allowed options");
    desc.add_options()("help", "produce help message")("debug", "view events for devices")("remap", "view events for devices");
    po::variables_map vm;
    po::store(po::parse_command_line(argc, argv, desc), vm);
    po::notify(vm);
    if (vm.count("help"))
    {
        std::cout << desc << "\n";
        return 0;
    }

    if (vm.count("debug"))
    {
        int idx = 0;
        auto devices = InputDevice::ListDevices();
        for (auto &device : devices)
        {
            std::cout << "[" << idx << "] " << device.Name() << std::endl;
            idx++;
        }

        std::cout << "Which devices should be watched?" << std::endl;

        int selectedIdx = 0;
        std::cin >> selectedIdx;
        while (selectedIdx < 0 && selectedIdx > devices.size())
        {
            std::cout << "invalid idx!" << std::endl;
        }

        auto device = std::move(devices[selectedIdx]);
        std::cout << "Selected: " << device.Name() << std::endl;
        std::cout << "Watching events..." << std::endl;

        while (true)
        {
            for (auto &event : device.ReadEvents())
            {
                auto eventType = event.type;
                if (eventType == InputEventType::ABS || eventType == InputEventType::SYN)
                {
                    continue;
                }

                std::cout << "type=" << as_integer(event.type) << ";code=" << as_integer(event.code) << ";value=" << event.value << std::endl;
            }
        }

        return 0;
    }

    if (vm.count("remap"))
    {
        int idx = 0;
        auto devices = InputDevice::ListDevices();
        for (auto &device : devices)
        {
            std::cout << "[" << idx << "] " << device.Name() << std::endl;
            idx++;
        }

        std::cout << "Which devices should be watched?" << std::endl;

        int selectedIdx = 0;
        std::cin >> selectedIdx;
        while (selectedIdx < 0 && selectedIdx > devices.size())
        {
            std::cout << "invalid idx!" << std::endl;
        }

        auto device = std::move(devices[selectedIdx]);
        std::cout << "Selected: " << device.Name() << std::endl;

        std::map<InputEventCode, InputEventCode> codeMap;
        std::cout << "Remapping started!" << std::endl;
        bool remapRunning = true;
        while (remapRunning)
        {
            std::cout << "Enter code to remap: ";
            int code;
            std::cin >> code;

            std::cout << "Enter code to be executed: ";
            int newCode;
            std::cin >> newCode;

            auto codeA = static_cast<InputEventCode>(code);
            auto codeB = static_cast<InputEventCode>(newCode);
            codeMap[codeA] = codeB;

            std::cout << "Continue? [Y/n] ";
            std::string msg;
            std::cin >> msg;
            if (msg == "n")
            {
                remapRunning = false;
                break;
            }
        }

        std::cout << "Name of virtual input? ";
        std::string name;
        getline(std::cin, name);

        auto caps = CodeMapToCapabilities(codeMap);
        OutputDevice outputDevice = OutputDevice::Create(name, caps);
        std::cout << "Created Device!";

        InputMap inputMap(device, outputDevice);
        inputMap.CodeMap(codeMap);
        inputMap.Start();

        return 0;
    }

#if 0
    std::vector<toml::value> maps = toml::find<std::vector<toml::value>>(data, "maps");
    for (auto &map : maps)
    {
        auto from =  toml::find<toml::value>(map, "from");
        auto fromPath =  toml::find<std::string>(from, "path");

        auto to =  toml::find<toml::value>(map, "to");
        auto toPath =  toml::find<std::string>(to, "path");

        InputMap inputMap(fromPath, toPath);
        inputMap.Start();
    }
#endif

    std::ofstream out("config.toml");
    out << data;
    out.close();

    return 0;
}
