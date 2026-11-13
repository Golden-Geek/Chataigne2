"""File publication to independent SDK-copy completion; no display/decoded-pixel oracle."""
from collections import Counter, defaultdict
from fractions import Fraction
import json
from pathlib import Path
import runpy

BOUNDS = runpy.run_path(str(Path(__file__).with_name('age_bounds.py')))['age_upper_bounds']


def key(value):
    return json.dumps(value, sort_keys=True)


def stamp(value):
    if type(value) is not int or not 0 < value < 1 << 64:
        raise ValueError('invalid Video age QPC coordinate')
    return value


def image_key(image):
    if image is None:
        return None
    frame = image['frame']
    if type(frame) is not int or not 0 <= frame < 1 << 64:
        raise ValueError('invalid SDK frame counter')
    return stamp(image['shared_handle']), frame


def analyze_video_age(outputs, uses, layouts, output_sources, receivers, receiver_clocks, window, frames,
                      *, output_streams=None, output_routes=None):
    frequency = stamp(window['frequency'])
    by_token, by_image = defaultdict(list), defaultdict(list)
    routes = defaultdict(list)
    for route in output_routes or []:
        routes[(key(route['generation']), route['output'])].append(route)
    for use in uses:
        by_token[key(use['token'])].append(use)
    for output in outputs:
        image = image_key(output.get('spout_image'))
        if output['outcome'] == 'Accepted' and image is not None:
            by_image[(output['output'], image)].append(output)
    completed = defaultdict(list)
    for output, observed in receivers.items():
        for receiver in observed:
            image = image_key(receiver.get('spout_image'))
            if image is not None:
                completed[(output, image)].append(receiver)
    result = []
    for identity, layout in output_sources.items():
        measured, missing, accepted = [], Counter(), 0
        for output in outputs:
            if output['output'] != identity or output['outcome'] != 'Accepted':
                continue
            token = key(output['token'])
            api = output.get('consumer_api')
            wall = frames.get(token, {}).get('wall')
            at = api['begin'] if api is not None else wall['begin'] if wall is not None else None
            if at is None:
                accepted += 1
                missing['unassignable_phase'] += 1
                continue
            if not window['begin_qpc'] <= at < window['end_qpc']:
                continue
            accepted += 1
            candidates = [use for use in by_token[token] if layouts.get(use.get('stream')) == layout
                          and (output_streams is None or use.get('stream') in output_streams.get(identity, set()))]
            reason = None
            if output_routes is not None:
                generation = frames.get(token, {}).get('generation')
                facts = routes[(key(generation), identity)] if generation is not None else []
                textures = {row['texture'] for row in facts if row['texture'] is not None}
                counts = {row['source_count'] for row in facts}
                if (len(counts) != 1 or next(iter(counts)) != len(textures) or not textures
                        or any(row['texture'] is None for row in facts)):
                    reason = 'missing_or_incomplete_output_sources'
                candidates = [use for use in candidates if use['texture'] in textures]
            # Equal-sized independent sources are ambiguous. Aliases must retain the same
            # exact producer/publication and frame metadata, not merely a matching timestamp.
            publications = {(use.get('stream'), use['sequence'], key(use.get('frame'))) for use in candidates}
            if reason is None and (not candidates or len(publications) != 1):
                reason = 'missing_or_ambiguous_source'
            elif reason is None and any(use.get('frame') is None or use['frame'].get('publication_qpc') is None for use in candidates):
                reason = 'missing_file_publication'
            image = image_key(output.get('spout_image'))
            if reason is None and (image is None or len(by_image[(identity, image)]) != 1):
                reason = 'missing_or_ambiguous_output_version'
            if reason is None and receiver_clocks.get(identity) != frequency:
                reason = 'missing_or_mismatched_receiver_contract'
            witnesses = completed[(identity, image)] if image is not None else []
            endpoints = [row for row in witnesses if row.get('completion_observed_qpc') is not None
                         and row.get('receive_begin_qpc') is not None]
            if reason is None and not endpoints:
                reason = 'missing_receiver_completion'
            if reason is None and api is None:
                reason = 'missing_output_api'
            if reason is not None:
                missing[reason] += 1
                continue
            published = stamp(candidates[0]['frame']['publication_qpc'])
            api_begin = stamp(api['begin'])
            ends = []
            for receiver in endpoints:
                receive, end = stamp(receiver['receive_begin_qpc']), stamp(receiver['completion_observed_qpc'])
                if published > api_begin or receive > end or api_begin > end:
                    raise ValueError('Video publication/output/receiver clock order is reversed')
                ends.append(end)
            # The tag precedes commit; the independent event poll follows completed capture.
            # Outward one-tick uncertainty at each endpoint keeps the rational bound conservative.
            measured.append(Fraction((min(ends)-published+2)*1000, frequency))
        result.append({'output': identity, 'layout': layout, 'missing_witnesses': dict(missing),
                       'publication_to_consumer_copy_upper_ms': BOUNDS(measured, accepted),
                       'coverage_complete': len(measured) == accepted and accepted > 0})
    return {
        'outputs': result,
        'scope': ('Q06 single-source SDK outputs, joined by File/stream when supplied; extent-only legacy '
                  'captures retain ambiguity. Publication pre-commit through '
                  'independent GPU capture completion, including polling delay. Unassignable '
                  'accepted rows contribute an unbounded endpoint in every phase.'),
        'limits': ('unmapped outputs, File publications without valid timestamp tags, decoded Video pixel/PTS validation, '
                   'display scanout and the complete Q06 format/repeated series remain unqualified'),
    }
