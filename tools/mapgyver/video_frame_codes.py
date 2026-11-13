"""Exact SDK-version pixel codes versus the actually selected File PTS; no timing pass inferred."""
from collections import Counter, defaultdict
import json


def key(value):
    return json.dumps(value, sort_keys=True)


def integer(value, label, minimum=0):
    if type(value) is not int or value < minimum:
        raise ValueError(f'invalid {label}')
    return value


def image(value):
    if value is None:
        return None
    return (integer(value['shared_handle'], 'SDK handle', 1), integer(value['frame'], 'SDK frame'))


def decoded_pixels(row, marker):
    words = row.get('raw_words')
    decoded = row.get('binary_frame_code')
    if not isinstance(words, list) or len(words) != 12 or any(type(v) is not int or not 0 <= v < 1 << 32 for v in words):
        return None
    bits, threshold = marker['bits'], marker['threshold']
    if words[0] != 0x56464331 or words[3] != bits:
        return None
    if any((words[2] >> (8 * lane)) & 255 < threshold for lane in range(4)):
        return None
    code = 0
    for bit in range(32):
        luminance = (words[4 + bit // 4] >> ((bit % 4) * 8)) & 255
        if bit < bits:
            code |= (luminance >= threshold) << bit
        elif luminance:
            return None
    if decoded != {'Ok': code} or type(decoded.get('Ok')) is not int or words[1] != code:
        return None
    return code


def analyze_frame_codes(events, receivers, fixtures, contracts, window, frames):
    uses, routes, published, copied = (defaultdict(list) for _ in range(4))
    for row in events['SourceUse']:
        integer(row['texture'], 'source texture', 1)
        uses[key(row['token'])].append(row)
    for row in events['OutputSource']:
        if row['texture'] is not None:
            integer(row['texture'], 'prepared texture', 1)
        routes[(key(row['generation']), row['output'])].append(row)
    for row in events['Output']:
        if row['outcome'] == 'Accepted' and image(row.get('spout_image')) is not None:
            published[(row['output'], image(row['spout_image']))].append(row)
    for output, rows in receivers.items():
        for row in rows:
            if image(row.get('spout_image')) is not None:
                copied[(output, image(row['spout_image']))].append(row)
    results = []
    for output, fixture in fixtures.items():
        rate, count = integer(fixture['rate'], 'fixture rate', 1), integer(fixture['frames'], 'fixture count', 1)
        bits = integer(fixture['marker']['bits'], 'marker bits', 1)
        if bits > 32 or not 0 < integer(fixture['marker']['threshold'], 'threshold') < 255:
            raise ValueError('invalid binary marker count or threshold')
        accepted, matched, invalid, mismatched = 0, 0, 0, []
        missing, distinct = Counter(), set()
        for row in events['Output']:
            if row['output'] != output or row['outcome'] != 'Accepted':
                continue
            batch = frames.get(key(row['token']))
            at = row.get('consumer_api') or (batch or {}).get('wall')
            if at is not None and not window['begin_qpc'] <= integer(at['begin'], 'output phase QPC', 1) < window['end_qpc']:
                continue
            accepted += 1
            reason, selected = None, []
            if at is None:
                reason = 'unassignable_phase'
            elif contracts.get(output) != fixture['marker']:
                reason = 'missing_or_mismatched_binary_contract'
            facts = routes.get((key(batch['generation']), output), []) if batch is not None else []
            counts = {integer(f['source_count'], 'source count') for f in facts}
            textures = {f['texture'] for f in facts if f['texture'] is not None}
            if reason is None and (len(counts) != 1 or next(iter(counts)) != len(textures) or not textures
                                   or any(f['texture'] is None for f in facts)):
                reason = 'missing_or_incomplete_output_sources'
            if reason is None:
                selected = [u for u in uses[key(row['token'])] if u['texture'] in textures and u.get('frame') is not None]
                identities = {(u.get('stream'), u['sequence'], key(u['frame'])) for u in selected}
                if len(identities) != 1 or any(u.get('stream') is None for u in selected):
                    reason = 'missing_or_ambiguous_selected_source'
                elif any(u.get('descriptor', {}).get('extent') != {'width': fixture['width'], 'height': fixture['height']}
                         for u in selected):
                    reason = 'missing_or_mismatched_source_descriptor'
            version = image(row.get('spout_image'))
            if reason is None and (version is None or len(published[(output, version)]) != 1):
                reason = 'missing_or_ambiguous_output_version'
            witnesses = copied[(output, version)]
            if reason is None and len(witnesses) != 1:
                reason = 'missing_or_ambiguous_pixel_witness'
            if reason is None and any(witnesses[0].get(field) is None for field in
                                      ('receive_begin_qpc', 'completion_observed_qpc')):
                reason = 'missing_receiver_completion'
            if reason is not None:
                missing[reason] += 1
                continue
            witness = witnesses[0]
            receive = integer(witness['receive_begin_qpc'], 'receive QPC', 1)
            end = integer(witness['completion_observed_qpc'], 'completion QPC', 1)
            if receive > end or at['begin'] > end:
                raise ValueError('pixel witness clock order is reversed')
            code = decoded_pixels(witness, fixture['marker'])
            if code is None:
                invalid += 1
                missing['invalid_binary_marker'] += 1
                continue
            pts = integer(selected[0]['frame']['pts_nanos'], 'selected File PTS')
            # Frame-numbered constant-rate fixtures start at PTS0. Round the nanosecond
            # quantization; floor would misidentify1/30s as frame0.
            ordinal = (pts * rate + 500_000_000) // 1_000_000_000
            if ordinal >= count or abs(pts * rate - ordinal * 1_000_000_000) > rate:
                raise ValueError('selected PTS is outside the declared numbered fixture')
            expected = ordinal % (1 << bits)
            if code != expected:
                mismatched.append({'token': row['token'], 'image': row['spout_image'], 'pts_nanos': pts,
                                   'expected': expected, 'observed': code, 'receiver_row': witness['row']})
            else:
                matched += 1
                distinct.add((pts, code))
        results.append({'output': output, 'accepted_images': accepted, 'matched_images': matched,
                        'distinct_pts_codes': len(distinct), 'invalid_markers': invalid,
                        'mismatched_images': len(mismatched), 'mismatch_examples': mismatched[:10],
                        'missing_witnesses': dict(missing),
                        'coverage_complete': accepted > 0 and matched == accepted})
    return {'outputs': results,
            'scope': 'Exact accepted SDK versions joined to prepared output/source facts and actual retained File PTS; all missing/ambiguous images remain unverified. No display/age/performance pass.'}
