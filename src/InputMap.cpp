#include "InputMap.h"

void InputMap::DoWork()
{
    while (true)
    {
        auto inputEvents = this->from.ReadEvents();
        for (auto &event : inputEvents)
        {
            if (this->codeMap.count(event.code))
            {
                auto mappedEvent = this->codeMap[event.code];
                InputEvent newEvent(event.type, mappedEvent, event.value);

                this->to.Write(newEvent);
            }
        }
    }
}

void InputMap::Start()
{
    DoWork();
}

void InputMap::Stop()
{
}
