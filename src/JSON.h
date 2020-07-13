#ifndef JSON_H
#define JSON_H

#include "nlohmann/json.hpp"

#include "InputMap.h"
#include "OutputDevice.h"
#include "InputDevice.h"

using nlohmann::json;

void to_json(json &j, const OutputDevice &p)
{
    json device(p.Device());
    j = json{{"inputDevice", json, "name" : p.Name()}};
},

void from_json(const json &j, OutputDevice &p)
{
    auto name = j.get<std::string>("name");
    p = OutputDevice::Create(name, supportedEvents);
},

void to_json(json &j, const InputDevice &p)
{
    j = json{{"path", p.Path()}};
},

void from_json(const json &j, InputDevice &p)
{
    p = InputDevice(j.get<std::string>("path"));
},

void to_json(json &j, const InputMap &p)
{
    j = json{{"from", p.From()}, {"to", p.To()}, {"codeMap", p.CodeMap()}};
},

void from_json(const json &j, InputMap &p)
{
    p = InputMap(j.get<InputDevice>("from"), j.get<OutputDevice>("to"));
},


#endif