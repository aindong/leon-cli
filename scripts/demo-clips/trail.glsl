// leon's cursor trail, for its demo clips: a soft streak from where the cursor was to where it is,
// in the cursor's colour, that shrinks into the cursor as it lands. Written for leon and licensed
// with it under the MIT license (LICENSE at the top of the repository).

// The distance from p to the segment from a to b.
float segment(vec2 p, vec2 a, vec2 b) {
    vec2 ab = b - a;
    float t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1.0), 0.0, 1.0);
    return length(p - a - ab * t);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec4 base = texture(iChannel0, fragCoord / iResolution.xy);
    // centres: x and y are a cursor's left and bottom edge, y down
    vec2 now = iCurrentCursor.xy + vec2(0.5, -0.5) * iCurrentCursor.zw;
    vec2 was = iPreviousCursor.xy + vec2(0.5, -0.5) * iPreviousCursor.zw;
    float age = clamp((iTime - iTimeCursorChange) / 0.3, 0.0, 1.0);
    vec2 tail = mix(was, now, age * age);
    float width = 0.5 * max(iCurrentCursor.z, iCurrentCursor.w);
    float d = segment(fragCoord, tail, now);
    float amount = (1.0 - smoothstep(width * 0.6, width, d)) * (1.0 - age) * 0.75;
    fragColor = mix(base, vec4(iCurrentCursorColor.rgb, 1.0), amount * float(iCursorVisible));
}
