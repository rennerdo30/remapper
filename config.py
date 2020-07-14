import json
import os
from pathlib import Path

import remap
import inputdevice
import outputdevice
import util

class Config:
    CONFIG_FILE = str(Path.home()) + '/.config/remapper/config.json'

    def __init__(self):
        self.data = {}
        if os.path.exists(self.CONFIG_FILE):
            with open(self.CONFIG_FILE) as json_file:
                self.data = json.load(json_file)
        else:
            path = os.path.dirname(os.path.abspath(self.CONFIG_FILE))
            os.makedirs(path)
            self.save()

    def save(self):
        with open(self.CONFIG_FILE, 'w') as outfile:
            json.dump(self.data, outfile)

    def add_remapper(self, remapper):
        if 'remappers' not in self.data:
            self.data['remappers'] = []
        self.data['remappers'].append(remapper.to_dict())
        self.save()

    def remove_remapper(self, remapper):
        if 'remappers' not in self.data:
            self.data['remappers'] = []
        to_delete = None
        for val in self.data['remappers']:
            if val['outputdevice']['name'] == remapper.outputdevice.name:
                to_delete = val
                break

        if to_delete is not None:
            self.data['remappers'].remove(to_delete)

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