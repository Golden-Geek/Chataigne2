import copy
from pathlib import Path
import runpy
import unittest

ANALYZE = runpy.run_path(str(Path(__file__).parents[1] / 'production_allocations.py'))['analyze_allocations']


def descriptor():
    return {'extent': {'width': 4, 'height': 4}, 'format': 'Bgra8Unorm'}


def inventory():
    return {'plan': {'created': 1, 'released': 0, 'live': 1, 'peak_live': 1,
                     'texel_bytes': 64, 'peak_texel_bytes': 64},
            'sources': [{'binding': 'source', 'bridge_slots': 3, 'descriptor': descriptor(),
                         'owned_texel_bytes': 192, 'import_aliases': 3, 'reader_leases': 2}],
            'outputs': [{'output': 1, 'generation': 1, 'allocation': {
                'Window': {'descriptor': descriptor(), 'buffers': 2}}},
                        {'output': 2, 'generation': 1, 'allocation': {
                            'Spout': {'descriptor': descriptor(), 'shared_handle': 12}}}],
            'owned_texel_bytes': 448, 'driver_physical_bytes': None}


def empty():
    value = inventory()
    value['plan'].update(released=1, live=0, texel_bytes=0)
    value['sources'][0].update(bridge_slots=0, descriptor=None, owned_texel_bytes=0,
                              import_aliases=0, reader_leases=0)
    value['outputs'] = []
    value['owned_texel_bytes'] = 0
    return value


class NativeAllocationTests(unittest.TestCase):
    def test_actual_owners_return_without_charging_aliases_or_driver_bytes(self):
        result = ANALYZE([inventory()], empty(), 512)
        self.assertIs(result['observed_within_target'], True)
        self.assertEqual(result['known_owned_texel_peak_bytes'], 448)
        self.assertEqual(result['maximum_uncharged_import_alias_owners'], 3)
        self.assertIsNone(result['driver_physical_allocation_bytes'])

    def test_partial_sdk_or_missing_metadata_remains_unknown_not_zero(self):
        value = inventory()
        value['outputs'][1]['allocation'] = 'UnknownSdk'
        value['owned_texel_bytes'] = None
        self.assertIsNone(ANALYZE([value], empty(), 512)['observed_within_target'])
        self.assertIsNone(ANALYZE([None], empty(), 512)['observed_within_target'])
        self.assertIsNone(ANALYZE([inventory()], None, 512)['observed_within_target'])

    def test_alias_or_reader_retention_cannot_hide_behind_zero_payload(self):
        for field in ('import_aliases', 'reader_leases'):
            value = empty()
            value['sources'][0][field] = 1
            self.assertFalse(ANALYZE([inventory()], value, 512)['observed_within_target'])
        self.assertFalse(ANALYZE([inventory()], empty(), 400)['observed_within_target'])

    def test_duplicate_charges_inconsistent_lifetimes_and_pool_overflow_are_rejected(self):
        for change in ('double_charge', 'lifetime', 'source_duplicate', 'pool_overflow', 'output_duplicate'):
            value = inventory()
            if change == 'double_charge':
                value['owned_texel_bytes'] += 64
            elif change == 'lifetime':
                value['plan']['released'] = 2
            elif change == 'source_duplicate':
                value['sources'].append(copy.deepcopy(value['sources'][0]))
            elif change == 'pool_overflow':
                value['sources'][0]['bridge_slots'] = 4
            else:
                value['outputs'].append(copy.deepcopy(value['outputs'][0]))
            with self.assertRaises(ValueError):
                ANALYZE([value], empty(), 512)


if __name__ == '__main__':
    unittest.main()
