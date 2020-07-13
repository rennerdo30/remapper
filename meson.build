project('remapper', 'c', 'cpp', default_options : ['c_std=c18', 'cpp_std=c++17'])

evdev = dependency('libevdev', version : '1.9.0')
nlohmann = dependency('nlohmann_json')
boost_dep = dependency('boost', modules : ['thread', 'program_options'])
magic_enum = declare_dependency(include_directories: 'lib/magic_enum')

project_source_files = [
  'src/main.cpp',
  'src/InputDevice.cpp',
  'src/KeyCodes.cpp',
  'src/InputMap.cpp',
  'src/OutputDevice.cpp'
]

executable('remapper', project_source_files, dependencies:[evdev, nlohmann, boost_dep, magic_enum])
