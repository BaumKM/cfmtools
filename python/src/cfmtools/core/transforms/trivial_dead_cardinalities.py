from cfmtools.core.cfm import CFM, FeatureList


def eliminate_trivial_dead_cardinalities(cfm: CFM) -> CFM:
    """
    Remove cheap / local dead cardinalities by imposing safe upper bounds.

    Enforced bounds:
      (1) max(group_instance(f)) <= sum_c max(feature_instance(c))
      (2) max(feature_instance(c)) <= max(group_instance(parent(c)))
      (3) max(group_type(f)) <= |children(f)|
    """
    feature_instance_cardinalities = FeatureList(cfm.feature_instance_cardinalities)
    group_instance_cardinalities = FeatureList(cfm.group_instance_cardinalities)
    group_type_cardinalities = FeatureList(cfm.group_type_cardinalities)

    # (3) max group-type: <= number of children
    for feature in cfm.features():
        max_types = len(cfm.children[feature])
        new_group_type = group_type_cardinalities[feature].bound(max_types)
        if new_group_type != group_type_cardinalities[feature]:
            group_type_cardinalities[feature] = new_group_type

    changed = True

    while changed:
        changed = False
        for feature in cfm.features():
            # (1) group-instance bounded by sum of children capacity
            child_max_sum = 0
            unbounded = False

            for c in cfm.children[feature]:
                c_max = feature_instance_cardinalities[c].max
                if c_max is None:
                    unbounded = True
                    break
                child_max_sum += c_max

            if not unbounded:
                new_group_instance = group_instance_cardinalities[feature].bound(
                    child_max_sum
                )
                if new_group_instance != group_instance_cardinalities[feature]:
                    group_instance_cardinalities[feature] = new_group_instance
                    changed = True

            # (2) feature-instance bounded by group instance
            parent = cfm.parents[feature]
            if parent is not None:
                parent_max = group_instance_cardinalities[parent].max
                if parent_max is not None:
                    new_feature_instance = feature_instance_cardinalities[
                        feature
                    ].bound(
                        parent_max,
                    )
                    if new_feature_instance != feature_instance_cardinalities[feature]:
                        feature_instance_cardinalities[feature] = new_feature_instance
                        changed = True

    return cfm.change_cardinalities(
        new_feature_instance_cardinalities=feature_instance_cardinalities,
        new_group_instance_cardinalities=group_instance_cardinalities,
        new_group_type_cardinalities=group_type_cardinalities,
    )
