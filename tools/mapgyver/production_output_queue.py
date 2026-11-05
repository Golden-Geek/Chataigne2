"""Native publication capacity bounds; never infer exact occupancy or consumption timestamps."""


def analyze_output_queue(frames, outputs, *, healthy):
    def key(token):
        device = token['device']
        return device['runtime'], device['generation'], token['serial']

    def positive(value):
        return type(value) is int and 0 < value < (1 << 32)

    indexed = {}
    for row in outputs:
        identity = key(row['token']), row['output']
        if identity in indexed:
            raise ValueError('duplicate output publication record')
        indexed[identity] = row
    identities = [key(frame['token']) for frame in frames]
    if len(set(identities)) != len(identities):
        raise ValueError('duplicate publication frame token')
    reports = []
    for output in (1, 2, 3):
        accepted, covered, maximum, unknown = 0, 0, None, []
        incarnations = {}
        for token in identities:
            row = indexed.get((token, output))
            if row is None:
                unknown.append('output attempt missing')
                continue
            if row['outcome'] != 'Accepted':
                if row.get('publication_queue') is not None:
                    raise ValueError('rejected output reports an accepted publication bound')
                continue
            accepted += 1
            queue, api = row.get('publication_queue'), row.get('consumer_api')
            if not isinstance(queue, dict) or len(queue) != 1 or not isinstance(api, dict) \
                    or not all(type(api.get(field)) is int and 0 < api[field] < (1 << 64)
                               for field in ('begin', 'end')) or api['end'] < api['begin']:
                unknown.append('accepted publication lacks native bound/API bracket')
                continue
            if output == 2:
                image = queue.get('SpoutLatestImage', {}).get('image')
                if image is None or image != row.get('spout_image') or not all(
                        positive(image.get(field)) for field in ('shared_handle', 'frame')) \
                        or image['frame'] >= (1 << 31):
                    unknown.append('latest-image bound lacks matching actual shared version')
                    continue
                bound = 1
            else:
                window, present = queue.get('Window'), row.get('window_present')
                if not window or not present or not positive(present.get('generation')) \
                        or not isinstance(present.get('addresses'), list) \
                        or len(present['addresses']) != 3 \
                        or not all(type(address) is int and 0 < address < (1 << 64)
                                   for address in present['addresses']):
                    unknown.append('window bound lacks exact native incarnation')
                    continue
                if window.get('admission_permit') is not True \
                        or window.get('sync_interval') not in (0, 1) or type(window.get('sync_interval')) is not int \
                        or not positive(window.get('buffer_count')) \
                        or window['buffer_count'] < 2 \
                        or not positive(window.get('maximum_frame_latency')) \
                        or window['maximum_frame_latency'] != 1:
                    unknown.append('window storage/readiness/presentation bound unverified')
                    continue
                incarnation = token[:2], present['generation'], tuple(present['addresses'])
                layout = window['buffer_count'], window['maximum_frame_latency'], window['sync_interval']
                if incarnation in incarnations and incarnations[incarnation] != layout:
                    raise ValueError('native window layout changes within one incarnation')
                incarnations[incarnation] = layout
                # Both exercised intervals keep the same actual storage and per-chain queue
                # limit. Include every back buffer instead of guessing DWM consumption.
                bound = window['buffer_count']
            covered += 1
            maximum = bound if maximum is None else max(maximum, bound)
        complete = healthy and accepted > 0 and covered == accepted and not unknown
        sufficient = complete and maximum <= 2
        reports.append({'output': output, 'accepted_publications': accepted,
                        'covered_publications': covered, 'coverage_complete': complete,
                        'maximum_verified_capacity_frames': maximum,
                        'capacity_sufficient_for_target': sufficient,
                        'exact_pending_frames': None, 'unknown_reasons': sorted(set(unknown))})
    sufficient = bool(frames) and all(row['capacity_sufficient_for_target'] for row in reports)
    return {'coverage_complete': bool(frames) and all(row['coverage_complete'] for row in reports),
            'observed_within_target': True if sufficient else None,
            'required_maximum_frames': 2, 'outputs': reports,
            'scope': 'verified app/native publication capacity bounds; not exact occupancy, GPU queue time or scanout',
            'limitations': ['larger/unknown capacity does not prove an actual occupancy violation',
                            'external receiver-owned storage is not an app-owned publication queue',
                            'GPU critical path and physical consumption timestamps remain separate requirements']}
