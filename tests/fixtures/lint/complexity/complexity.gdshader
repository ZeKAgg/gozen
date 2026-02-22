shader_type spatial;

void helper(float a, float b, float c, float d, float e, float f, float g, float h, float i, float j) {
    if (a > 0.0 && b > 0.0 && c > 0.0) {
        while (d > 0.0) {
            if (e > 0.0) {
                for (int n = 0; n < 10; n++) {
                    if (f > 0.0) {
                        break;
                    }
                }
            } else if (g > 0.0) {
                while (h > 0.0) {
                    if (i > 0.0) {
                        continue;
                    }
                }
            }
        }
    } else if (j > 0.0) {
        if (a > 1.0) {
            if (b > 1.0) {
                if (c > 1.0) {
                    ALBEDO = vec3(1.0);
                }
            }
        }
    }
}
