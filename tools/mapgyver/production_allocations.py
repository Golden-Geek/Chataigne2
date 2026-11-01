"""Validate sampled native owner payloads without inventing physical driver allocations."""


def analyze_allocations(inventories, final, ceiling):
    def number(value):
        if type(value) is not int or not 0 <= value < (1 << 64):
            raise ValueError('invalid native allocation counter')
        return value

    def descriptor(value):
        if value is None:
            return None
        if value.get('format') not in ('Rgba8Unorm', 'Bgra8Unorm'):
            raise ValueError('unsupported native allocation texel format')
        width, height = (number(value['extent'][key]) for key in ('width', 'height'))
        if not width or not height:
            raise ValueError('empty native allocation descriptor')
        return number(width * height * 4)

    def inspect(value):
        if value is None:
            return {'known': False, 'bytes': None, 'aliases': None, 'empty': None}
        plan = value['plan']
        live, payload = number(plan['live']), number(plan['texel_bytes'])
        if number(plan['created']) - number(plan['released']) != live \
                or number(plan['peak_live']) < live or number(plan['peak_texel_bytes']) < payload:
            raise ValueError('native plan owner counters contradict their lifetime')
        total, known, aliases, empty = payload, True, 0, live == payload == 0
        identities = set()
        for source in value['sources']:
            if source['binding'] in identities:
                raise ValueError('duplicate native source allocation owner')
            identities.add(source['binding'])
            slots, imported, readers = (number(source[key]) for key in ('bridge_slots', 'import_aliases', 'reader_leases'))
            if slots > 3 or imported > 6 or readers > 3:
                raise ValueError('native source ownership exceeds fixed pool capacity')
            size = descriptor(source['descriptor'])
            actual = 0 if slots == 0 else size * slots if size is not None else None
            if actual != source['owned_texel_bytes']:
                raise ValueError('source bridge payload differs from its committed pool')
            known = known and actual is not None
            total += actual or 0
            aliases += imported
            empty = empty and slots == imported == readers == 0
        identities = set()
        for output in value['outputs']:
            if output['output'] in identities:
                raise ValueError('duplicate native output allocation owner')
            identities.add(output['output'])
            allocation = output['allocation']
            if allocation is None:
                continue
            empty = False
            if allocation == 'UnknownSdk':
                known = False
                continue
            if len(allocation) != 1:
                raise ValueError('ambiguous native output allocation')
            if 'Window' in allocation:
                layout = allocation['Window']
                buffers = number(layout['buffers'])
                if buffers == 0:
                    raise ValueError('window owner has no buffers')
                total += descriptor(layout['descriptor']) * buffers
            elif 'Spout' in allocation:
                layout = allocation['Spout']
                if not 0 < number(layout['shared_handle']) < (1 << 32):
                    raise ValueError('invalid owned Spout shared-image identity')
                total += descriptor(layout['descriptor'])
            else:
                raise ValueError('unknown native output allocation kind')
        expected = total if known else None
        if expected != value['owned_texel_bytes']:
            raise ValueError('native owned payload duplicates aliases or omits storage')
        return {'known': known, 'bytes': expected, 'aliases': aliases, 'empty': empty}

    samples = [inspect(value) for value in inventories]
    after = inspect(final)
    known = [row['bytes'] for row in samples if row['known']]
    complete = bool(samples) and len(known) == len(samples)
    returned = after['empty'] if after['known'] else None
    return {'sampled_snapshots': len(samples), 'known_snapshots': len(known),
            'metadata_coverage_complete': complete,
            'known_owned_texel_peak_bytes': max(known) if known else None,
            'maximum_uncharged_import_alias_owners': max((r['aliases'] for r in samples if r['aliases'] is not None), default=None),
            'final_owner_inventory_empty': returned,
            'observed_within_target': max(known) <= ceiling and returned if complete and returned is not None else None,
            'driver_physical_allocation_bytes': None,
            'scope': 'sampled actual native owners and validated texel payload; aliases uncharged; construction transients and driver allocation/residency remain unmeasured'}
