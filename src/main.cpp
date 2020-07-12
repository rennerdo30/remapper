#include <iostream>

#include <toml.hpp>
#include <boost/program_options.hpp>

#include "InputDevice.h"
#include "InputMap.h"

namespace po = boost::program_options;

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
                std::cout << "type=" << event.type << ";code=" << event.code << ";value=" << event.value << std::endl;
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
        bool remapRunning = true;
        std::cout << "Remapping started! Enter 'exit' to stop.";
        while (remapRunning)
        {
            std::cout << "Continue? ";
            std::string msg;
            std::cin >> msg;
            if (msg == "exit")
            {
                remapRunning = false;
                break;
            }

            std::cout << "Enter code to remap: ";
            int code;
            std::cin >> code;

            std::cout << "Enter code to be executed: ";
            int newCode;
            std::cin >> newCode;

            auto codeA = static_cast<InputEventCode>(code);
            auto codeB = static_cast<InputEventCode>(newCode);
            codeMap.insert(codeA, codeB);
        }

        std::cout << "Name of virtual input? ";
        std::string name;

        auto caps = CodeMapToCapabilities(codeMap);
        OutputDevice outputDevice = OutputDevice::Create(name, caps);
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
