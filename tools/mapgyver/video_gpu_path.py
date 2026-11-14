"""File GPU dependencies joined by actual output routes and immutable producer tokens."""
from collections import Counter, defaultdict
import json
from pathlib import Path
import runpy

GPU_PATH = runpy.run_path(str(Path(__file__).with_name('production_gpu_path.py')))


def key(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'))


def unique(rows, identity):
    result = {}
    for row in rows:
        value = identity(row)
        if value in result:
            raise ValueError('duplicate Video GPU-path identity')
        result[value] = row
    return result


def analyze_video_gpu_path(frames, native, bindings, frequency, distribution, *, healthy):
    routes, outputs = defaultdict(list), defaultdict(list)
    unique(frames, lambda row: key(row['token']))
    unique(native['OutputSource'], lambda row: (key(row['generation']), row['output'], row['texture']))
    unique(native['Output'], lambda row: (key(row['token']), row['output']))
    for row in native['OutputSource']:
        routes[(key(row['generation']), row['output'])].append(row)
    for row in native['Output']:
        outputs[key(row['token'])].append(row['output'])
    submissions = unique(native['SourceSubmission'], lambda row: key(row['token']))
    streams = {row['stream'] for row in bindings}
    expected, selected, source_free = {}, [], 0
    for frame in frames:
        identity = key(frame['token'])
        missing, textures = [], set()
        drawn = outputs[identity]
        if not drawn:
            missing.append('renderer output observations missing')
        for output in drawn:
            facts = routes[(key(frame['generation']), output)]
            counts = {row['source_count'] for row in facts}
            actual = {row['texture'] for row in facts if row['texture'] is not None}
            valid = (len(counts) == 1 and all(type(value) is int and value >= 0 for value in counts)
                     and next(iter(counts)) == len(actual)
                     and all(type(value) is int and value > 0 for value in actual)
                     and (all(row['texture'] is not None for row in facts) if actual
                          else len(facts) == 1 and facts[0]['texture'] is None))
            if not valid:
                missing.append('missing/incomplete actual output source table')
            textures.update(actual)
        if not textures and not missing:
            source_free += 1
            continue
        expected[identity] = textures, missing
        selected.append(frame)

    def dependencies(frame, uses, sources):
        identity = key(frame['token'])
        textures, reasons = expected[identity]
        missing, result, seen = list(reasons), [], set()
        for texture in sorted(textures):
            use = uses.get((identity, texture))
            publication = use.get('publication') if use else None
            origin = key(publication.get('source_token')) if publication else None
            source, submission = sources.get(origin), submissions.get(origin)
            stream = use.get('stream') if use else None
            if (not source or not submission or stream not in streams
                    or type(use.get('sequence')) is not int or use['sequence'] <= 0
                    or source.get('context') != {'FrameProducer': {'stream': stream}}
                    or submission.get('stream') != stream
                    or not isinstance(use.get('descriptor'), dict)
                    or submission.get('descriptor') != use['descriptor']):
                missing.append(f'exact File producer dependency missing/foreign for texture {texture}')
                continue
            # Aliases/References retain one actual submission, not one charge per texture.
            if origin not in seen:
                result.append((source, source['context']))
                seen.add(origin)
        return result, missing

    result = GPU_PATH['analyze_gpu_path'](selected, native, frequency, distribution,
                                         healthy=healthy, resolve_dependencies=dependencies,
                                         include_context_gaps=False)
    reasons = Counter(reason for row in result['records'] for reason in row['unknown_reasons'])
    result.update(input_renderer_batches=len(frames), excluded_proven_source_free_batches=source_free,
                  missing_reasons=dict(reasons),
                  scope='Actual File-dependent output batches, exact generation/texture routes and retained producer '
                        'tokens; full renderer span includes its other rendering/output work. Only proven '
                        'source-free batches are excluded and counted separately.')
    result['limitations'].append('Uninstrumented render-thread File uploads and foreign/unbound sources remain '
                                 'incomplete; equal dimensions/PTS never substitute for producer identity.')
    result['limitations'].append('Intentional File-frame retention contributes only to the conservative upper '
                                 'enclosure. Failure uses the largest quantization-corrected local GPU span; '
                                 'retention gaps and serial span sums cannot prove slow GPU execution.')
    return result
