"""Outward age bounds over every accepted image, including unmeasured endpoints."""
from fractions import Fraction
import math


def age_upper_bounds(values, total):
    if type(total) is not int or total < 0 or len(values) > total \
            or any(not isinstance(value, Fraction) or value < 0 for value in values):
        raise ValueError('invalid whole-cohort age observations')
    ordered = sorted(values)
    ranks = {name: (total * percentile + 99) // 100
             for name, percentile in (('median', 50), ('p95', 95), ('p99', 99), ('maximum', 100))}
    bounds = {name: ordered[rank-1] if 0 < rank <= len(ordered) else None
              for name, rank in ranks.items()}

    def upper_float(value):
        if value is None:
            return None
        approximate = float(value)
        return math.nextafter(approximate, math.inf) if Fraction(approximate) < value else approximate

    p95 = bounds['p95']
    return {
        'accepted_images': total, 'finite_age_images': len(ordered),
        'unbounded_age_images': total-len(ordered), 'ranks': ranks,
        **{name+'_upper_ms': upper_float(value) for name, value in bounds.items()},
        'unbounded_statistics': [name for name, value in bounds.items() if value is None],
        'p95_upper_exact_ms': {'numerator': str(p95.numerator), 'denominator': str(p95.denominator)}
                              if p95 is not None else None,
        'p95_within_target': p95 <= 50 if p95 is not None else None,
        'p95_target_ms': 50,
        'scope': 'all accepted images; unknown ages have an unbounded upper endpoint',
    }
