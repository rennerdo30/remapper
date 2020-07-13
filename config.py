import json
import os

import remap
import inputdevice
import outputdevice
import util

class Config:
    def __init__(self):
        self.data = {}
        if os.path.exists('config.json'):
            with open('config.json') as json_file:
                self.data = json.load(json_file)
        else:
            self.save()

    def save(self):
        with open('config.json', 'w') as outfile:
            json.dump(self.data, outfile)

    def add_remapper(self, remapper):
        if 'remappers' not in self.data:
            self.data['remappers'] = []
        self.data['remappers'].append(remapper.to_dict())
        self.save()

    def get_remappers(self):
        if 'remappers' not in self.data:
            self.data['remappers'] = []

        remappers = []
        for remapper in self.data['remappers']:
            in_dev = inputdevice.from_json(remapper['inputdevice'])
            out_dev = outputdevice.from_json(remapper['outputdevice'])

            event_map = {}
            for key, value in remapper['event_map'].items():
                event_map[int(key)] = value

            mapper = remap.Remap(event_map, in_dev, out_dev, remapper['grab_device'])
            remappers.append(mapper)

        return remappers