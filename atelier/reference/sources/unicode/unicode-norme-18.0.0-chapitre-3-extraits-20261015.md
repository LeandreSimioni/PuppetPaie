# The Unicode Standard, Version 18.0 — chapitre 3, « Conformance » : extraits

Transcription (le texte de la page HTML, sections citées seulement) de
https://www.unicode.org/versions/Unicode18.0.0/core-spec/chapter-3/, relevée le 15/10/2026 (HTTP 200, `text/html`,
428 731 octets). La page n'est pas un fichier de données : les conditions d'Unicode (https://www.unicode.org/copyright.html)
n'en permettent la copie que « for personal or internal business purposes and not for public distribution ». L'original
va dans `reference-privee/sources/unicode/` du dépôt privé ; ici, les seuls passages que le moteur applique.

Notation de la page : `AC0016` se lit AC00 en base 16 (l'indice « 16 » est collé au nombre par la conversion en texte).

© 1991-2026 Unicode, Inc. Unicode and the Unicode Logo are registered trademarks of Unicode, Inc. in the U.S. and other
countries.

---

3.7 Decomposition
D62 Decomposition mapping: A mapping from a character to a sequence of one or more characters that is a canonical or compatibility equivalent, and that is listed in the character names list or described in Section 3.12, Conjoining Jamo Behavior.
Each character has at most one decomposition mapping. The mappings in Section 3.12, Conjoining Jamo Behavior, are canonical mappings. The mappings in the character names list are identified as either canonical or compatibility mappings (see Section 24.1, Character Names List).
D63 Decomposable character: A character that is equivalent to a sequence of one or more other characters, according to the decomposition mappings found in the Unicode Character Database, and those described in Section 3.12, Conjoining Jamo Behavior.
A decomposable character is also referred to as a precomposed character or composite character.
The decomposition mappings from the Unicode Character Database are also given in Section 24.1, Character Names List.
D64 Decomposition: A sequence of one or more characters that is equivalent to a decomposable character. A full decomposition of a character sequence results from decomposing each of the characters in the sequence until no characters can be further decomposed.
3.7.1 Compatibility Decomposition
D65 Compatibility decomposition: The decomposition of a character or character sequence that results from recursively applying both the compatibility mappings and the canonical mappings found in the Unicode Character Database, and those described in Section 3.12, Conjoining Jamo Behavior, until no characters can be further decomposed, and then reordering nonspacing marks according to Section 3.11, Normalization Forms.
The decomposition mappings from the Unicode Character Database are also given in Section 24.1, Character Names List.
Some compatibility decompositions remove formatting information.
D66 Compatibility decomposable character: A character whose compatibility decomposition is not identical to its canonical decomposition. It may also be known as a compatibility precomposed character or a compatibility composite character.
For example, U+00B5 MICRO SIGN has no canonical decomposition mapping, so its canonical decomposition is the same as the character itself. It has a compatibility decomposition to U+03BC GREEK SMALL LETTER MU. Because MICRO SIGN has a compatibility decomposition that is not equal to its canonical decomposition, it is a compatibility decomposable character.
For example, U+03D3 GREEK UPSILON WITH ACUTE AND HOOK SYMBOL canonically decomposes to the sequence <U+03D2 GREEK UPSILON WITH HOOK SYMBOL, U+0301 COMBINING ACUTE ACCENT>. That sequence has a compatibility decomposition of <U+03A5 GREEK CAPITAL LETTER UPSILON, U+0301 COMBINING ACUTE ACCENT>. Because GREEK UPSILON WITH ACUTE AND HOOK SYMBOL has a compatibility decomposition that is not equal to its canonical decomposition, it is a compatibility decomposable character.
This term should not be confused with the term “compatibility character,” which is discussed in Section 2.3, Compatibility Characters.
Many compatibility decomposable characters are included in the Unicode Standard solely to represent distinctions in other base standards. They support transmission and processing of legacy data. Their use is discouraged other than for legacy data or other special circumstances.
Some widely used and indispensable characters, such as NBSP, are compatibility decomposable characters for historical reasons. Their use is not discouraged.
A large number of compatibility decomposable characters are used in phonetic and mathematical notation, where their use is not discouraged.
For historical reasons, some characters that might have been given a compatibility decomposition were not, in fact, decomposed. The Normalization Stability Policy prohibits adding decompositions for such cases in the future, so that normalization forms will stay stable. See the Unicode Character Encoding Stability Policies.
Replacing a compatibility decomposable character by its compatibility decomposition may lose round-trip convertibility with a base standard.
D67 Compatibility equivalent: Two character sequences are said to be compatibility equivalents if their full compatibility decompositions are identical.

[…]

3.11 Normalization Forms
The concepts of canonical equivalent (D70) or compatibility equivalent (D67) characters in the Unicode Standard make it necessary to have a full, formal definition of equivalence for Unicode strings. String equivalence is determined by a process called normalization, whereby strings are converted into forms which are compared directly for identity.
This section provides the formal definitions of the four Unicode Normalization Forms. It defines the Canonical Ordering Algorithm and the Canonical Composition Algorithm which are used to convert Unicode strings to one of the Unicode Normalization Forms for comparison. It also formally defines Unicode Combining Classes—values assigned to all Unicode characters and used by the Canonical Ordering Algorithm.
Note: In versions of the Unicode Standard up to Version 5.1.0, the Unicode Normalization Forms and the Canonical Composition Algorithm were defined in Unicode Standard Annex #15, “Unicode Normalization Forms.” Those definitions have now been consolidated in this chapter, for clarity of exposition of the normative definitions and algorithms involved in Unicode normalization. However, because implementation of Unicode normalization is quite complex, implementers are still advised to fully consult Unicode Standard Annex #15, “Unicode Normalization Forms,” which contains more detailed explanations, examples, and implementation strategies.
Unicode normalization should be carefully distinguished from Unicode collation. Both processes involve comparison of Unicode strings. However, the point of Unicode normalization is to make a determination of canonical (or compatibility) equivalence or non-equivalence of strings—it does not provide any rank-ordering information about those strings. Unicode collation, on the other hand, is designed to provide orderable weights or “keys” for strings; those keys can then be used to sort strings into ordered lists. Unicode normalization is not tailorable; normalization equivalence relationships between strings are exact and unchangeable. Unicode collation, on the other hand, is designed to be tailorable to allow many kinds of localized and other specialized orderings of strings. For more information, see Unicode Technical Standard #10, “Unicode Collation Algorithm.”
D102 [Moved to Section 3.6, Combination and renumbered as D61a.]
D103 [Moved to Section 3.6, Combination and renumbered as D61b.]
3.11.1 Normalization Stability
A very important attribute of the Unicode Normalization Forms is that they must remain stable between versions of the Unicode Standard. A Unicode string normalized to a particular Unicode Normalization Form in one version of the standard is guaranteed to remain in that Normalization Form for implementations of future versions of the standard. In order to ensure this stability, there are strong constraints on changes of any character properties that are involved in the specification of normalization—in particular, the combining class and the decomposition of characters. The details of those constraints are spelled out in the Normalization Stability Policy. The requirement for stability of normalization also constrains what kinds of characters can be encoded in future versions of the standard. For an extended discussion of this topic, see Section 3, Versioning and Stability, in Unicode Standard Annex #15, “Unicode Normalization Forms.”
3.11.2 Combining Classes
Each character in the Unicode Standard has a combining class associated with it. The combining class is a numerical value used by the Canonical Ordering Algorithm to determine which sequences of combining marks are to be considered canonically equivalent and which are not. Canonical equivalence is the criterion used to determine whether two character sequences are considered identical for interpretation.
D104 Combining class: A numeric value in the range 0..254 given to each Unicode code point, formally defined as the property Canonical_Combining_Class.
The combining class for each encoded character in the standard is specified in the file UnicodeData.txt in the Unicode Character Database. Any code point not listed in that data file defaults to Canonical_Combining_Class=0 (or ccc=0 for short).
An extracted listing of combining classes, sorted by numeric value, is provided in the file DerivedCombiningClass.txt in the Unicode Character Database.
Only combining marks have a combining class other than zero. Almost all combining marks with a class other than zero are also nonspacing marks, with a few exceptions. Also, not all nonspacing marks have a non-zero combining class. Thus, while the correlation between \p{ccc≠0} and \p{gc=Mn} is close, it is not exact, and implementations should not depend on the two concepts being identical.
D105 Fixed position class: A subset of the range of numeric values for combining classes—specifically, any value in the range 10..199.
Fixed position classes are assigned to a small number of Hebrew, Arabic, Syriac, Telugu, Thai, Lao, and Tibetan combining marks whose positions were conceived of as occurring in a fixed position with respect to their grapheme base, regardless of any other combining mark that might also apply to the grapheme base.
Not all Arabic vowel points or Indic matras are given fixed position classes. The existence of fixed position classes in the standard is an historical artifact of an earlier stage in its development, prior to the formal standardization of the Unicode Normalization Forms.
D106 Typographic interaction: Graphical application of one nonspacing mark in a position relative to a grapheme base that is already occupied by another nonspacing mark, so that some rendering adjustment must be done (such as default stacking or side-by-side placement) to avoid illegible overprinting or crashing of glyphs.
The assignment of combining class values for Unicode characters was originally done with the goal in mind of defining distinct numeric values for each group of nonspacing marks that would typographically interact. Thus all generic nonspacing marks placed above the base character are given the same value, ccc=230, while all generic nonspacing marks placed below are given the value ccc=220. Nonspacing marks that tend to sit on one “shoulder” or another of a grapheme base, or that may actually be attached to the grapheme base itself when applied, have their own combining classes.
The design of canonical ordering generally assures that:
When two combining characters C1 and C2 do typographically interact, the sequence C1+ C2 is not canonically equivalent to C2+ C1.
When two combining characters C1 and C2 do not typographically interact, the sequence C1+ C2 is canonically equivalent to C2+ C1.
This is roughly correct for the normal cases of detached, generic nonspacing marks placed above and below base letters. However, the ramifications of complex rendering for many scripts ensure that there are always some edge cases involving typographic interaction between combining marks of distinct combining classes. This has turned out to be particularly true for some of the fixed position classes for Hebrew and Arabic, for which a distinct combining class is no guarantee that there will be no typographic interaction for rendering.
Because of these considerations, particular combining class values should be taken only as a guideline regarding issues of typographic interaction of combining marks.
The only normative use of combining class values is as input to the Canonical Ordering Algorithm, where they are used to normatively distinguish between sequences of combining marks that are canonically equivalent and those that are not.
3.11.3 Specification of Unicode Normalization Forms
The specification of Unicode Normalization Forms applies to all Unicode coded character sequences (D12). For clarity of exposition in the definitions and rules specified here, the terms “character” and “character sequence” are used, but coded character sequences refer also to sequences containing noncharacters or reserved code points. Unicode Normalization Forms are specified for all Unicode code points, and not just for ordinary, assigned graphic characters.
3.11.4 Starters
D107 Starter: Any code point (assigned or not) with combining class of zero (ccc = 0).
Note that ccc = 0 is the default value for the Canonical_Combining_Class property, so that all reserved code points are Starters by definition. Noncharacters are also Starters by definition. All control characters, format characters, and private-use characters are also Starters.
Private agreements cannot override the value of the Canonical_Combining_Class property for private-use characters.
Among the graphic characters, all those with General_Category values other than gc = M are Starters. Some combining marks have ccc = 0 and thus are also Starters. Combining marks with ccc other than 0 are not Starters. Table 3-14 summarizes the relationship between types of combining marks and their status as Starters.
Table 3-14. Combining Marks and Starter Status Description gc ccc Starter
Nonspacing Mn 0 Yes
> 0 No
Spacing Mc 0 Yes
> 0 No
Enclosing Me 0 Yes

The term Starter refers, in concept, to the starting character of a combining character sequence (D56), because all combining character sequences except defective combining character sequences (D57) commence with a ccc = 0 character—in other words, they start with a Starter. However, because the specification of Unicode Normalization Forms must apply to all possible coded character sequences, and not just to typical combining character sequences, the behavior of a code point for Unicode Normalization Forms is specified entirely in terms of its status as a Starter or a non-starter, together with its Decomposition_Mapping value.
3.11.5 Canonical Ordering Algorithm
D108 Reorderable pair: Two adjacent characters A and B in a coded character sequence <A, B> are a Reorderable Pair if and only if ccc(A) > ccc(B) > 0.
D109 Canonical Ordering Algorithm: In a decomposed character sequence D, exchange the positions of the characters in each Reorderable Pair until the sequence contains no more Reorderable Pairs.
In effect, the Canonical Ordering Algorithm is a local bubble sort that guarantees that a Canonical Decomposition or a Compatibility Decomposition will contain no subsequences in which a combining mark is followed directly by another combining mark that has a lower, non-zero combining class.
Canonical ordering is defined in terms of application of the Canonical Ordering Algorithm to an entire decomposed sequence. For example, canonical decomposition of the sequence <U+1E0B LATIN SMALL LETTER D WITH DOT ABOVE, U+0323 COMBINING DOT BELOW> would result in the sequence <U+0064 LATIN SMALL LETTER D, U+0307 COMBINING DOT ABOVE, U+0323 COMBINING DOT BELOW>, a sequence which is not yet in canonical order. Most decompositions for Unicode strings are already in canonical order.
Table 3-15 gives some examples of sequences of characters, showing which of them constitute a Reorderable Pair and the reasons for that determination. Except for the base character “a”, the other characters in the example table are combining marks; character names are abbreviated in the Sequence column to make the examples clearer.
Table 3-15. Reorderable Pairs Sequence Combining Classes Reorderable? Reason
<a, acute> 0, 230 No ccc(A) = 0
<acute, a> 230, 0 No ccc(B) = 0
<diaeresis, acute> 230, 230 No ccc(A) = ccc(B)
<cedilla, acute> 202, 230 No ccc(A) < ccc(B)
<acute, cedilla> 230, 202 Yes ccc(A) > ccc(B)

3.11.6 Canonical Composition Algorithm
D110 Singleton decomposition: A canonical decomposition mapping from a character to a different single character.
The default value for the Decomposition_Mapping property for a code point (including any private-use character, any noncharacter, and any unassigned code point) is the code point itself. This default value does not count as a singleton decomposition, because it does not map a character to a different character. Private agreements cannot override the decomposition mapping for private-use characters.
Example: U+2126 OHM SIGN has a singleton decomposition to U+03A9 GREEK CAPITAL LETTER OMEGA.
A character with a singleton decomposition is often referred to simply as a singleton for short.
D110a Expanding canonical decomposition: A canonical decomposition mapping from a character to a sequence of more than one character.
D110b Starter decomposition: An expanding canonical decomposition for which both the character being mapped and the first character of the resulting sequence are Starters.
Definitions D110a and D110b are introduced to simplify the following definition of non-starter decomposition and make it more precise.
D111 Non-starter decomposition: An expanding canonical decomposition which is not a starter decomposition.
Example: U+0344 COMBINING GREEK DIALYTIKA TONOS has an expanding canonical decomposition to the sequence <U+0308 COMBINING DIAERESIS, U+0301 COMBINING ACUTE ACCENT>. U+0344 is a non-starter, and the first character in its decomposition is a non-starter. Therefore, on two counts, U+0344 has a non-starter decomposition.
Example: U+0F73 TIBETAN VOWEL SIGN II has an expanding canonical decomposition to the sequence <U+0F71 TIBETAN VOWEL SIGN AA, U+0F72 TIBETAN VOWEL SIGN I>. The first character in that sequence is a non-starter. Therefore U+0F73 has a non-starter decomposition, even though U+0F73 is a Starter.
As of the current version of the standard, there are no instances of the third possible situation: a non-starter character with an expanding canonical decomposition to a sequence whose first character is a Starter.
D112 Composition exclusion: A Canonical Decomposable Character (D69) which has the property value Composition_Exclusion = True.
The list of Composition Exclusions is provided in CompositionExclusions.txt in the Unicode Character Database.
D113 Full composition exclusion: A Canonical Decomposable Character which has the property value Full_Composition_Exclusion = True.
Full composition exclusions consist of the entire list of composition exclusions plus all characters with singleton decompositions or with non-starter decompositions.
For convenience in implementation of Unicode normalization, the derived property Full_Composition_Exclusion is computed, and all characters with the property value Full_Composition_Exclusion = True are listed in DerivedNormalizationProps.txt in the Unicode Character Database.
D114 Primary composite: A Canonical Decomposable Character (D69) which is not a Full Composition Exclusion.
For any given version of the Unicode Standard, the list of primary composites can be computed by extracting all canonical decomposable characters from UnicodeData.txt in the Unicode Character Database, adding the list of precomposed Hangul syllables (D132), and subtracting the list of Full Composition Exclusions.
D115 Blocked: Let A and C be two characters in a coded character sequence <A, ... C>. C is blocked from A if and only if ccc(A) = 0 and there exists some character B between A and C in the coded character sequence, i.e., <A, ... B, ... C>, and either ccc(B) = 0 or ccc(B) >= ccc(C).
Because the Canonical Composition Algorithm operates on a string which is already in canonical order, testing whether a character is blocked requires looking only at the immediately preceding character in the string.
D116 Non-blocked pair: A pair of characters <A, ... C> in a coded character sequence, in which C is not blocked from A.
It is important for proper implementation of the Canonical Composition Algorithm to be aware that a Non-blocked Pair need not be contiguous.
D117 Canonical Composition Algorithm: Starting from the second character in the coded character sequence (of a Canonical Decomposition or Compatibility Decomposition) and proceeding sequentially to the final character, perform the following steps:
R1 Seek back (left) in the coded character sequence from the character C to find the last Starter L preceding C in the character sequence.
R2 If there is such an L, and C is not blocked from L, and there exists a Primary Composite P which is canonically equivalent to the sequence <L, C>, then replace L by P in the sequence and delete C from the sequence.
When the algorithm completes, all Non-blocked Pairs canonically equivalent to a Primary Composite will have been systematically replaced by those Primary Composites.
The replacement of the Starter L in R2 requires continuing to check the succeeding characters until the character at that position is no longer part of any Non-blocked Pair that can be replaced by a Primary Composite. For example, consider the following hypothetical coded character sequence: <U+007A z, U+0335 short stroke overlay, U+0327 cedilla, U+0324 diaeresis below, U+0301 acute>. None of the first three combining marks forms a Primary Composite with the letter z. However, the fourth combining mark in the sequence, acute, does form a Primary Composite with z, and it is not Blocked from the z. Therefore, R2 mandates the replacement of the sequence <U+007A z, ... U+0301 acute> with <U+017A z-acute, ...>, even though there are three other combining marks intervening in the sequence.
The character C in R1 is not necessarily a non-starter. It is necessary to check all characters in the sequence, because there are sequences <L, C> where both L and C are Starters, yet there is a Primary Composite P which is canonically equivalent to that sequence. For example, Indic two-part vowels often have canonical decompositions into sequences of two spacing vowel signs, each of which has Canonical_Combining_Class = 0 and which is thus a Starter by definition. Nevertheless, such a decomposed sequence has an equivalent Primary Composite.
3.11.7 Definition of Normalization Forms
The Unicode Standard specifies four normalization forms. Informally, two of these forms are defined by maximal decomposition of equivalent sequences, and two of these forms are defined by maximal composition of equivalent sequences. Each is then differentiated based on whether it employs a Canonical Decomposition or a Compatibility Decomposition.
D118 Normalization Form D (NFD): The Canonical Decomposition of a coded character sequence.
D119 Normalization Form KD (NFKD): The Compatibility Decomposition of a coded character sequence.
D120 Normalization Form C (NFC): The Canonical Composition of the Canonical Decomposition of a coded character sequence.
D121 Normalization Form KC (NFKC): The Canonical Composition of the Compatibility Decomposition of a coded character sequence.
Logically, to get the NFD or NFKD (maximally decomposed) normalization form for a Unicode string, one first computes the full decomposition of that string and then applies the Canonical Ordering Algorithm to it.
Logically, to get the NFC or NFKC (maximally composed) normalization form for a Unicode string, one first computes the NFD or NFKD normalization form for that string, and then applies the Canonical Composition Algorithm to it.

[…]

3.12 Conjoining Jamo Behavior
The Unicode Standard contains both a large set of precomposed modern Hangul syllables and a set of conjoining Hangul jamo, which can be used to encode archaic Korean syllable blocks as well as modern Korean syllable blocks. This section describes how to
Determine the canonical decomposition of precomposed Hangul syllables.
Compose jamo characters into precomposed Hangul syllables.
Algorithmically determine the names of precomposed Hangul syllables.
For more information, see the “Hangul Syllables” and “Hangul Jamo” subsections in Section 18.6, Hangul. Hangul syllables are a special case of grapheme clusters. For the algorithm to determine syllable boundaries in a sequence of conjoining jamo characters, see Section 8, “Hangul Syllable Boundary Determination” in Unicode Standard Annex #29, “Unicode Text Segmentation.”
3.12.1 Definitions
The following definitions use the Hangul_Syllable_Type property, which is defined in the UCD file HangulSyllableType.txt.
D122 Leading consonant: A character with the Hangul_Syllable_Type property value Leading_Jamo. Abbreviated as L.
When not occurring in clusters, the term leading consonant is equivalent to syllable-initial character.
D123 Choseong: A sequence of one or more leading consonants.
In Modern Korean, a choseong consists of a single jamo. In Old Korean, a sequence of more than one leading consonant may occur.
Equivalent to syllable-initial cluster.
D124 Choseong filler: U+115F HANGUL CHOSEONG FILLER. Abbreviated as Lf.
A choseong filler stands in for a missing choseong to make a well-formed Korean syllable.
D125 Vowel: A character with the Hangul_Syllable_Type property value Vowel_Jamo. Abbreviated as V.
When not occurring in clusters, the term vowel is equivalent to syllable-peak character.
D126 Jungseong: A sequence of one or more vowels.
In Modern Korean, a jungseong consists of a single jamo. In Old Korean, a sequence of more than one vowel may occur.
Equivalent to syllable-peak cluster.
D127 Jungseong filler: U+1160 HANGUL JUNGSEONG FILLER. Abbreviated as Vf.
A jungseong filler stands in for a missing jungseong to make a well-formed Korean syllable.
D128 Trailing consonant: A character with the Hangul_Syllable_Type property value Trailing_Jamo. Abbreviated as T.
When not occurring in clusters, the term trailing consonant is equivalent to syllable-final character.
D129 Jongseong: A sequence of one or more trailing consonants.
In Modern Korean, a jongseong consists of a single jamo. In Old Korean, a sequence of more than one trailing consonant may occur.
Equivalent to syllable-final cluster.
D130 LV_Syllable: A character with Hangul_Syllable_Type property value LV_Syllable. Abbreviated as LV.
An LV_Syllable has a canonical decomposition to a sequence of the form <L, V>.
D131 LVT_Syllable: A character with Hangul_Syllable_Type property value LVT_Syllable. Abbreviated as LVT.
An LVT_Syllable has a canonical decomposition to a sequence of the form <LV, T>.
D132 Precomposed Hangul syllable: A character that is either an LV_Syllable or an LVT_Syllable.
D133 Syllable block: A sequence of Korean characters that should be grouped into a single square cell for display.
This is different from a precomposed Hangul syllable and is meant to include sequences needed for the representation of Old Korean syllables.
A syllable block may contain a precomposed Hangul syllable plus other characters.
D134 Standard Korean syllable block: A sequence of one or more L followed by a sequence of one or more V and a sequence of zero or more T, or any other sequence that is canonically equivalent.
All precomposed Hangul syllables, which have the form LV or LVT, are standard Korean syllable blocks.
Alternatively, a standard Korean syllable block may be expressed as a sequence of a choseong and a jungseong, optionally followed by a jongseong.
A choseong filler may substitute for a missing leading consonant, and a jungseong filler may substitute for a missing vowel.
This definition is used in Unicode Standard Annex #29, “Unicode Text Segmentation,” as part of the algorithm for determining syllable boundaries in a sequence of conjoining jamo characters.
3.12.2 Hangul Syllable Decomposition
The following algorithm specifies how to take a precomposed Hangul syllable s and arithmetically derive its full canonical decomposition d. This normative mapping for precomposed Hangul syllables is referenced by D68, Canonical decomposition, in Section 3.7, Decomposition.
This algorithm, as well as the other Hangul-related algorithms defined in the following text, is first specified in pseudo-code. Then each is exemplified, showing its application to a particular Hangul character or sequence. The Hangul characters used in those examples are shown in Table 3-16. Finally, each algorithm is then further exemplified with an implementation as a Java method at the end of this section.
Table 3-16. Hangul Characters Used in Examples Code Point Glyph Character Name Jamo Short Name
U+D4DB 퓛 HANGUL SYLLABLE PWILH
U+1111 ᄑ HANGUL CHOSEONG PHIEUPH P
U+1171 ᅱ HANGUL JUNGSEONG WI WI
U+11B6 ᆶ HANGUL JONGSEONG RIEUL-HIEUH LH

Common Constants. Define the following constants:

SBase = AC0016
LBase = 110016
VBase = 116116
TBase = 11A716
LCount = 19
VCount = 21
TCount = 28
NCount = 588 (VCount * TCount)
SCount = 11172 (LCount * NCount)

TBase is set to one less than the beginning of the range of trailing consonants, which starts at U+11A8. TCount is set to one more than the number of trailing consonants relevant to the decomposition algorithm: (11C216 - 11A816 + 1) + 1. NCount is thus the number of precomposed Hangul syllables starting with the same leading consonant, counting both the LV_Syllables and the LVT_Syllables for each possible trailing consonant. SCount is the total number of precomposed Hangul syllables.
Syllable Index. First compute the index of the precomposed Hangul syllable s:

SIndex = s - SBase

Arithmetic Decomposition Mapping. If the precomposed Hangul syllable s with the index SIndex (defined above) has the Hangul_Syllable_Type value LV, then it has a canonical decomposition mapping into a sequence of an L jamo and a V jamo, <LPart, VPart>:

LIndex = SIndex div NCount
VIndex = (SIndex mod NCount) div TCount
LPart = LBase + LIndex
VPart = VBase + VIndex

If the precomposed Hangul syllable s with the index SIndex (defined above) has the Hangul_Syllable_Type value LVT, then it has a canonical decomposition mapping into a sequence of an LV_Syllable and a T jamo, <LVPart, TPart>:

LVIndex = (SIndex div TCount) * TCount
TIndex = SIndex mod TCount
LVPart = SBase + LVIndex
TPart = TBase + TIndex

In this specification, the “div” operator refers to integer division (rounded down). The “mod” operator refers to the modulo operation, equivalent to the integer remainder for positive numbers.
The canonical decomposition mappings calculated this way are equivalent to the values of the Unicode character property Decomposition_Mapping (dm), for each precomposed Hangul syllable.
Full Canonical Decomposition. The full canonical decomposition for a Unicode character is defined as the recursive application of canonical decomposition mappings. The canonical decomposition mapping of an LVT_Syllable contains an LVPart which itself is a precomposed Hangul syllable and thus must be further decomposed. However, it is simplest to unwind the recursion and directly calculate the resulting <LPart, VPart, TPart> sequence instead. For full canonical decomposition of a precomposed Hangul syllable, compute the indices and components as follows:

LIndex = SIndex div NCount
VIndex = (SIndex mod NCount) div TCount
TIndex = SIndex mod TCount
LPart = LBase + LIndex
VPart = VBase + VIndex
TPart = TBase + TIndex if TIndex > 0

If TIndex = 0, then there is no trailing consonant, so map the precomposed Hangul syllable s to its full decomposition d = <LPart, VPart>. Otherwise, there is a trailing consonant, so map s to its full decomposition d = <LPart, VPart, TPart>.
Example. For the precomposed Hangul syllable U+D4DB, compute the indices and components:

SIndex = 10459
LIndex = 17
VIndex = 16
TIndex = 15
LPart = LBase + 17 = 111116
VPart = VBase + 16 = 117116
TPart = TBase + 15 = 11B616

Then map the precomposed syllable to the calculated sequence of components, which constitute its full canonical decomposition:
