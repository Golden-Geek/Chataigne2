"""Replay a spatial correspondence check from retained native images and pointer metadata."""
import argparse
import json
from pathlib import Path

from motion_frames import match_native_controls


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    root = parser.parse_args().directory
    load = lambda name: json.loads((root/name).read_text(encoding='utf8'))
    report = load('native-motion.json')
    result = match_native_controls(report['physical']['samples'], report['native']['samples'],
                                   load('motion-start/capture.json'), load('motion-start/geometry.json'))
    (root/'control-correspondence.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf8')
    print(json.dumps({k: v for k, v in result.items() if k != 'samples'}))
    return 0 if result['all_sampled_controls_spatially_matched'] else 2


if __name__ == '__main__':
    raise SystemExit(main())
