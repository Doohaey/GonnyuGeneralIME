package io.gannyu.input;

/** Pure geometry used by KeyboardRowLayout and its dependency-free boundary test. */
final class HorizontalKeyHitResolver {
    private HorizontalKeyHitResolver() {}

    static int nearestKeyIndex(int x, int[] lefts, int[] rights) {
        if (lefts.length != rights.length) {
            throw new IllegalArgumentException("left/right key bounds must have equal length");
        }
        if (lefts.length == 0) return -1;

        int nearestIndex = 0;
        int nearestDistance = distanceFrom(x, lefts[0], rights[0]);
        for (int index = 1; index < lefts.length; index++) {
            final int distance = distanceFrom(x, lefts[index], rights[index]);
            if (distance < nearestDistance) {
                nearestIndex = index;
                nearestDistance = distance;
            }
        }
        return nearestIndex;
    }

    private static int distanceFrom(int x, int left, int right) {
        if (right <= left) {
            throw new IllegalArgumentException("key bounds must have positive width");
        }
        if (x < left) return left - x;
        if (x >= right) return x - right + 1;
        return 0;
    }
}
