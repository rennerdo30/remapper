#ifndef INPUTMAP_H
#define INPUTMAP_H

#include <memory>
#include <vector>
#include <map>
#include <thread>
#include <toml.hpp>

#include <libevdev/libevdev.h>

#include "InputDevice.h"
#include "OutputDevice.h"

class InputMap
{
public:
    InputMap(const InputDevice &from, const OutputDevice &to) : from(from), to(to){};
    InputMap(const InputMap &other) : from(other.from), to(other.to), codeMap(other.codeMap){};

    void Start();
    void Stop();

    std::map<InputEventCode, InputEventCode> CodeMap() const { return this->codeMap; }
    void CodeMap(const std::map<InputEventCode, InputEventCode> &codeMap) { this->codeMap = codeMap; }

private:
    InputDevice from;
    OutputDevice to;
    std::map<InputEventCode, InputEventCode> codeMap;
    
    void DoWork();
};

#endif // INPUTMAP_H
