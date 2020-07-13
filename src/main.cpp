#include <iostream>

#include <boost/program_options.hpp>
#include "nlohmann/json.hpp"

#include "InputDevice.h"
#include "InputMap.h"
#include "KeyCodes.h"

#include <filesystem>
#include <fstream>

namespace po = boost::program_options;
using nlohmann::json;

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

struct Config
{
    json data;

    Config()
    {
        if (!exists("config.json"))
        {
            std::ofstream output("config.json");
            output.close();

            data = json::object();
        }
        else
        {

            std::ifstream ifs("config.json");
            data = json::parse(ifs);
        }
    }

    ~Config()
    {
        std::ofstream file("config.json");
        file << data;
    }
};

void Debug(Config &config)
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
}

void Remap(Config &config)
{

    int idx = 0;
    auto devices = InputDevice::ListDevices();
    for (auto &device : devices)
    {
        std::cout << "[" << idx << "] " << device.Name() << std::endl;
        idx++;
    }

    std::cout << "Which devices should be watched?" << std::endl;

    std::string msg;
    getline(std::cin, msg);
    int selectedIdx = std::stoi(msg);
    while (selectedIdx < 0 && selectedIdx > devices.size())
    {
        std::cout << "invalid idx!" << std::endl;

        getline(std::cin, msg);
        selectedIdx = std::stoi(msg);
    }

    auto device = std::move(devices[selectedIdx]);
    std::cout << "Selected: " << device.Name() << std::endl;

    std::map<InputEventCode, InputEventCode> codeMap;
    std::cout << "Remapping started!" << std::endl;
    bool remapRunning = true;
    while (remapRunning)
    {
        std::string msg;
        std::cout << "Enter code to remap: ";
        getline(std::cin, msg);
        int code = std::stoi(msg);

        std::cout << "Enter code to be executed: ";
        getline(std::cin, msg);
        int newCode = std::stoi(msg);

        auto codeA = static_cast<InputEventCode>(code);
        auto codeB = static_cast<InputEventCode>(newCode);
        codeMap[codeA] = codeB;

        std::cout << "Continue? [Y/n] ";
        getline(std::cin, msg);
        if (msg == "n")
        {
            remapRunning = false;
            break;
        }
    }

    std::cout << "Name of virtual input? ";
    std::string name;
    getline(std::cin, name);

    std::cout << "Creating " << name << "..." << std::endl;

    auto caps = CodeMapToCapabilities(codeMap);
    OutputDevice outputDevice = OutputDevice::Create(name, caps);
    std::cout << "Created Device!" << std::endl;

    InputMap inputMap(device, outputDevice);
    inputMap.CodeMap(codeMap);

    std::thread runner([&inputMap]() { inputMap.Start(); });
    runner.join();
}

int main(int argc, char **argv)
{
    Config config;

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
        Debug(config);
        return 0;
    }

    if (vm.count("remap"))
    {
        Remap(config);
        return 0;
    }

    return 0;
}
