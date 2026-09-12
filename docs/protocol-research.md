# Read-only X protocol research

Observed **2026-09-10 UTC**. Research only: public HTTP source/metadata retrieval and static inspection; no Bird installation/execution, browser-profile access, Keychain access, guest activation, or authenticated API requests. No session credentials or public bearer value are reproduced. Local history was unavailable (`ctx status`: generation verification failure); no history repair attempted.

## Conclusions

- Current public X JavaScript supplies concrete read-query definitions, including **`Viewer` for authenticated identity**. These are source-verified, **not live authenticated interoperability tests**.
- Prefer `TweetResultByRestId` for a single post, `TweetDetail` for conversation context. Do not infer complete reply trees.
- Bird is useful historical reference, not a current protocol authority or required dependency. **One Bird search fallback ID is currently explicitly a mutation in X's bundle. Never copy its fallback list.**
- macOS Chrome's inspected current Chromium implementation still supports Keychain-derived `v10` AES-CBC. Cookie DB schema 24 requires verifying the decrypted host digest. This does not establish compatibility with every released Chrome channel/profile or grant access through OS protection.

## 1. Provenance and license

Canonical author/project: **Peter Steinberger, `steipete/bird`, npm `@steipete/bird`**. The author's [profile README at `7f8fd020b386013f35256cb3c94c8809a97c37da`](https://github.com/steipete/steipete/blob/7f8fd020b386013f35256cb3c94c8809a97c37da/README.md) links `steipete/bird` and says “had to make it private.” The unauthenticated [repository API](https://api.github.com/repos/steipete/bird) returned 404; 404 alone cannot distinguish private/deleted repositories.

Public release examined:

- [npm metadata](https://registry.npmjs.org/@steipete/bird/0.8.0): version **0.8.0**, published **2026-01-19T07:17:30.221Z**; recorded `gitHead` **`6ae239383c0692b0dc96ea07de42d5b17d6ee2a3`**. Git source at that revision was not independently accessible/verified. Metadata now marks the package deprecated/unsupported and does not supply a license field.
- [Exact package archive](https://registry.npmjs.org/@steipete/bird/-/bird-0.8.0.tgz): fetched and inspected in memory, never executed/extracted as an installation. SHA-1 verified against registry: **`a32c34074c2799646b07bb8b25b379d59f57b870`**. Registry integrity: `sha512-p7+a9a/olzf1Rxe56a51VMFoBlFQpFVosC5B8dB3rOT8UbSZ3Ey5eXCZoLDjKXDf8xKINvSmGtMAd3yjeE4Gcw==` (metadata; not independently signature-verified).
- Archive `package/LICENSE` explicitly says **MIT License, Copyright (c) 2025 Peter Steinberger**, with the standard permission/notice/disclaimer text. [Version-pinned readable copy](https://unpkg.com/@steipete/bird@0.8.0/LICENSE). Preserve the copyright and permission notice if copying substantial code; audit dependencies separately. Missing npm license metadata is not evidence that the shipped MIT license is absent.
- [jawond/bird at `c0d08f32352fd8e0063d69c9b57760ff4c940b11`](https://github.com/jawond/bird/tree/c0d08f32352fd8e0063d69c9b57760ff4c940b11) is an older public copy: inspected default-head commit dated 2025-12-05. Its public availability does **not** make it the canonical current upstream or prove equality with npm 0.8.0. `steipete/birdclaw` is a separate archive-oriented project.

Historical reference files below are version-pinned published JavaScript, not a claimed reconstruction of inaccessible TypeScript:

- [B-base](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-base.js), [B-constants](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-constants.js), [B-IDs](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/query-ids.json), [B-discovery](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/runtime-query-ids.js).
- [B-identity](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-users.js), [B-user](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-user-lookup.js), [B-timeline](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-user-tweets.js).
- [B-detail](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-tweet-detail.js), [B-search](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-search.js), [B-features](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-features.js), [B-parsing](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-utils.js).

## 2. Current first-party evidence

Public `https://x.com/explore`, `/notifications`, and `/settings/profile` returned logged-out HTML advertising **[X-main](https://abs.twimg.com/responsive-web/client-web/main.7313c8670b523249a.js)**. This is the primary source for all “current” definitions below:

- 1,122,841 bytes; SHA-256 **`f2c17811723ec0617ef6cf213fd56eb732d4f042c7ff81b7d72e33778f9ead65`**.
- `/` and `/?lang=en` instead advertised a different frontend: [entry-client-logged-out-BTHi9EQM.js](https://abs.twimg.com/x-web/x-web/entry-client-logged-out-BTHi9EQM.js), 22,043 bytes; SHA-256 **`917dde1567aa44f0b8c28f0db235f28037f31b3bea1cd15bb4dd070fd413b543`**. No target operation definitions were found in that entry file (its entire dependency graph was not searched).
- Consequence: a homepage-only, old-asset-path discovery regex is insufficient. Availability to this unauthenticated observer is not proof that every account/region receives the same rollout.

### Transport and identity

X-main's GraphQL adapter uses **GET for `operationType:"query"` unless explicitly forced to POST**, with URL-encoded JSON `variables`, `features`, and optional `fieldToggles`:

`https://x.com/i/api/graphql/<queryId>/<operationName>`

The adapter selects only declared feature-switch names and serializes each as `getValueWithoutScribeImpression(name) === true`; field toggles are filtered to the operation's declared names. Do not copy all features globally or interpret a listed switch as necessarily enabled.

**Concrete identity endpoint:**

`GET https://x.com/i/api/graphql/9t128XgFic52jPUEkJMf6w/Viewer`

Current X-main `fetchViewer` variables: `{withCommunitiesMemberships: <c9s_enabled boolean>}`. Field toggles supplied by the call site: `{isDelegate: <caller boolean>, withAuxiliaryUserLabels: <blue_business_multi_affiliates_ui_enabled boolean>}`. For the initial non-delegated account model, propose `isDelegate:false`; do not infer delegated identity support. Response: **`data.viewer.user_results.result`**, requiring `__typename:"User"`, nonempty string `rest_id`, and `core.screen_name` (legacy compatibility may check `legacy.screen_name`). Pin the stable `rest_id`, not the handle or GraphQL opaque `id`.

B-identity historically tries `https://x.com/i/api/account/settings.json`, then API-domain settings, then `https://x.com/i/api/account/verify_credentials.json?skip_status=true&include_entities=false` and `https://api.twitter.com/1.1/account/verify_credentials.json?skip_status=true&include_entities=false`. These are **historical candidates, not confirmed working identity endpoints**. X-main contains `account/settings` but not `verify_credentials`. Bird's parser accepts `user_id`, `user_id_str`, or nested `user.id_str`/string `user.id`, but notably **does not accept top-level `id_str`** from a conventional verify-credentials user object. Do not transplant this parser or its authenticated-settings-HTML scraping fallback. `UserByScreenName` verifies the requested profile, not the cookie owner's identity.

B-base documents the historical session envelope: the `auth_token` and `ct0` cookies, `x-csrf-token` matching that same session's `ct0`, authorization with the public web-client bearer, `x-twitter-auth-type: OAuth2Session`, active-user/client-language headers, origin/referer. These cookies are account-level authority, not read-only credentials. The public bearer alone is not user authentication. Browser-style additional headers and transaction-ID requirements remain an interoperability blocker: Bird generates random transaction bytes, whereas X-main loads its transaction implementation from a dynamic chunk. Random IDs are not established as valid current behavior. No authentication envelope was exercised here.

### Operation manifest

All six current definitions below declare **`operationType:"query"`** in X-main. Variables are statically observed call-site shapes, not a server-validated minimal schema. Omit optional cursor/context fields when absent. Retain IDs as strings.

| Operation / module | Current query ID | Variables and raw JSON response root |
|---|---|---|
| `Viewer` / 227618 | `9t128XgFic52jPUEkJMf6w` | See identity section; `data.viewer.user_results.result` |
| `TweetResultByRestId` / 728926 | `snmujSvB_9WXyd8yjvZ24Q` | Single-post no-replies branch: `{tweetId, withCommunity:false, includePromotedContent:false, withVoice:false}`; `data.tweetResult.result` |
| `TweetDetail` / 720832 | `FyR-GrebyjdkRoW1z6uCgQ` | `{focalTweetId, cursor?, referrer?, controller_data?, rux_context?, with_rux_injections?, rankingMode:"Relevance", includePromotedContent:true, withCommunity:<c9s_enabled>, withQuickPromoteEligibilityTweetFields:true, withBirdwatchNotes:<responsive_web_birdwatch_consumption_enabled>, withVoice:<voice_consumption_enabled>, isReaderMode?}`; `data.threaded_conversation_with_injections_v2.instructions` |
| `SearchTimeline` / 447423 | `KPSo2_UWdOMpPJwjhfT1Qg` | `{rawQuery, count, cursor?, querySource, product, withGrokTranslatedBio:<Top/People and translation switch>, withQuickPromoteEligibilityTweetFields:<caller === true>}`; `data.search_by_raw_query.search_timeline.timeline.instructions`. `product:"Latest"`, `querySource:"typed_query"`, count 20 are Bird's historical read-search choices. |
| `UserByScreenName` / 360985 | `KybxDj9RrADIITXlGG8kpw` | `{screen_name, withGrokTranslatedBio:<responsive_web_grok_bio_auto_translation_is_enabled>}`; `data.user.result`, then `rest_id`, `core.screen_name`, `core.name`; handle `UserUnavailable` |
| `UserTweets` / 859881 | `OeFjWKHutsuyWXZGmLr02A` | `{userId, count, cursor?, sortByMostLiked?, includePromotedContent:true, withQuickPromoteEligibilityTweetFields:true, withVoice:<voice_consumption_enabled>}`; `data.user.result.timeline.timeline.instructions` |

Several call sites also spread X-main module 891515's shared-variable helper. Its imported implementation was not fully traced; the table is not a claim that additional shared variables can never occur.

Historical IDs for comparison, **not fallback recommendations**:

- Bird shipped `TweetDetail: _NvJCnIjOW__EP5-RF197A` in B-IDs and `97JF30KziU00483E_8elBA` in constants; additional fallback `aFvUsJm2c-oDkJV75blV6g`. Its post-read path uses TweetDetail, not a dedicated single-post query.
- Bird shipped `SearchTimeline: 6AAys3t42mosm_yTI_QENg`; constants/fallbacks include `M1jEez78PEfVfbQLvlWMvQ`, `5h0kNbk3ii97rmfY6CdgAA`, `Tp1sewRU1AsZpBWhqCZicQ`. **X-main module 536462 maps `5h0kNbk3ii97rmfY6CdgAA` to `SharingAudiospacesListeningDataWithFollowersUpdate`, type `mutation`.** This proves the fallback manifest unsafe to assume correct; it does not prove a mismatched request would successfully mutate anything.
- Bird UserByScreenName candidates: `xc8f1g7BYqr6VTzTbvNlGw`, `qW5u-DAuXpMEG0zA1F7UGQ`, `sLVLhk0bGj3MVFEKTdax1w`; variables include historical `withSafetyModeUserFields:true` rather than the current translation flag.
- Bird UserTweets: `Wms1GvIiHXAPBaCr9KblaA`; historical `includePromotedContent:false`, `withVoice:true`, toggle `withArticlePlainText:false`.
- Bird search unusually POSTs `{features,queryId}` with variables in the URL; TweetDetail tries POST after GET 404. Neither behavior should replace the current query adapter's observed GET convention.

### Exact feature-switch sets

The following compact set notation preserves **all names** in current operation metadata. Values are deliberately not asserted to be universally correct: account experiments and logged-out configuration differ.

**Common post set P** — identical for UserTweets, TweetDetail, SearchTimeline:

```text
rweb_video_screen_enabled
rweb_cashtags_enabled
profile_label_improvements_pcf_label_in_post_enabled
responsive_web_profile_redirect_enabled
rweb_tipjar_consumption_enabled
verified_phone_label_enabled
creator_subscriptions_tweet_preview_api_enabled
responsive_web_graphql_timeline_navigation_enabled
premium_content_api_read_enabled
communities_web_enable_tweet_community_results_fetch
c9s_tweet_anatomy_moderator_badge_enabled
responsive_web_grok_analyze_button_fetch_trends_enabled
responsive_web_grok_analyze_post_followups_enabled
rweb_cashtags_composer_attachment_enabled
responsive_web_jetfuel_frame
rweb_sports_post_context_enabled
responsive_web_grok_share_attachment_enabled
responsive_web_grok_annotations_enabled
articles_preview_enabled
responsive_web_edit_tweet_api_enabled
rweb_conversational_replies_downvote_enabled
graphql_is_translatable_rweb_tweet_is_translatable_enabled
view_counts_everywhere_api_enabled
longform_notetweets_consumption_enabled
responsive_web_twitter_article_tweet_consumption_enabled
content_disclosure_indicator_enabled
content_disclosure_ai_generated_indicator_enabled
responsive_web_grok_show_grok_translated_post
responsive_web_grok_analysis_button_from_backend
post_ctas_fetch_enabled
freedom_of_speech_not_reach_fetch_enabled
standardized_nudges_misinfo
tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled
longform_notetweets_rich_text_read_enabled
longform_notetweets_inline_media_enabled
responsive_web_grok_image_annotation_enabled
responsive_web_grok_imagine_annotation_enabled
responsive_web_grok_community_note_auto_translation_is_enabled
responsive_web_enhance_cards_enabled
```

`TweetResultByRestId` = P minus `rweb_video_screen_enabled` and `responsive_web_enhance_cards_enabled` (ordering differs, membership otherwise identical).

`UserByScreenName` = `hidden_profile_subscriptions_enabled`, `profile_label_improvements_pcf_label_in_post_enabled`, `responsive_web_profile_redirect_enabled`, `rweb_tipjar_consumption_enabled`, `verified_phone_label_enabled`, `subscriptions_verification_info_is_identity_verified_enabled`, `subscriptions_verification_info_verified_since_enabled`, `highlights_tweets_tab_ui_enabled`, `responsive_web_twitter_article_notes_tab_enabled`, `subscriptions_feature_can_gift_premium`, `creator_subscriptions_tweet_preview_api_enabled`, `responsive_web_graphql_timeline_navigation_enabled`.

`Viewer` = `subscriptions_upsells_api_enabled`, `profile_label_improvements_pcf_label_in_post_enabled`, `responsive_web_profile_redirect_enabled`, `rweb_tipjar_consumption_enabled`, `verified_phone_label_enabled`, `creator_subscriptions_tweet_preview_api_enabled`, `responsive_web_graphql_timeline_navigation_enabled`.

Logged-out `/explore` HTML inspection found consistent literal `false` values for these P members: `rweb_video_screen_enabled`, `rweb_tipjar_consumption_enabled`, `verified_phone_label_enabled`, `premium_content_api_read_enabled`, `responsive_web_grok_analyze_button_fetch_trends_enabled`, `responsive_web_grok_analyze_post_followups_enabled`, `rweb_sports_post_context_enabled`, `post_ctas_fetch_enabled`, `longform_notetweets_inline_media_enabled`, `responsive_web_enhance_cards_enabled`; other P members had literal `true`, except `rweb_conversational_replies_downvote_enabled`, which was absent from that simple name/value scan. UserByScreenName's gift-premium switch had both false and true occurrences in separate configuration sections; Viewer upsells had false. These are **observations, not a resolved authenticated feature profile**. Missing flags must not be guessed from Bird's much older true/false dictionaries.

Field-toggle metadata for all four post operations: `withPayments`, `withAuxiliaryUserLabels`, `withArticleRichContentState`, `withArticlePlainText`, `withArticleSummaryText`, `withArticleVoiceOver`, `withGrokAnalyze`, `withDisallowedReplyControls`. UserByScreenName: first two only. Viewer: `isDelegate` plus first two. A declared toggle need not be supplied. Current TweetDetail call site explicitly computes `withArticlePlainText` as a switch **AND false**; Bird's article toggles enable rich content and plain text. Do not promise article completeness based on those historical flags.

### Parsing and pagination

X-main corroborates the top-level roots in the table and discriminates `Tweet`, `TweetWithVisibilityResults`, `TweetUnavailable`, and `UserUnavailable`. B-parsing supplies useful historical nested paths:

- `instructions[].entries[].content.itemContent.tweet_results.result`; module items at `content.items[].item.itemContent.tweet_results.result` (and direct item variants).
- Unwrap `TweetWithVisibilityResults.tweet` before mapping. Never turn unavailable/blocked/protected/deleted/tombstone results into an empty successful post.
- Post ID `rest_id`; author `core.user_results.result.rest_id` and its `core`/legacy profile fields; full note text `note_tweet.note_tweet_results.result.text` before `legacy.full_text`; conversation/reply IDs in `legacy.conversation_id_str` and `legacy.in_reply_to_status_id_str`; quote `quoted_status_result.result`; media from legacy entities/extended entities. Keep metrics optional.
- Bottom cursor: entry `content.cursorType === "Bottom"`, opaque `content.value`; support module/show-more and replacement instructions after fixture verification rather than only Bird's `entries[]` loop. Preserve ordering, deduplicate by stable ID, bound pages, detect repeated cursors, and report retained partial results on later failure.
- GraphQL HTTP 200 can contain errors and data together. Missing expected roots is schema drift/error, not necessarily “zero results.” Same-conversation membership and sorted timestamps do not prove a full thread or reconstruct parent edges.

### Follow-up: user-observed SearchTimeline HTTP 404

After the user reported HTTP 404 from the authenticated search command, public
sources were re-fetched without cookies, browser access or authenticated calls:

- `/explore` still advertises X-main above, whose SHA-256 is unchanged.
  `SearchTimeline` remains query ID `KPSo2_UWdOMpPJwjhfT1Qg`, type `query`.
  `fetchSearchGraphQL` calls the normal GraphQL adapter without `forcePost`.
- The adapter explicitly supplies **`content-type: application/json` on GET**.
  xcli omitted that header; it has now been aligned with the source contract.
  This discrepancy is established, but is **not yet proven to cause the 404**.
- [Vendor bundle](https://abs.twimg.com/responsive-web/client-web/vendor.588f26613fb657c8a.js),
  SHA-256 `121fd93d17747b914c5593917e14cc2c2f12e6849f7ebd4ac8589afdedcd6ad9`,
  supplies module `812055` as an empty object. Thus the shared-variable helper
  inspected earlier does not add missing search variables in this snapshot.
- [Logged-in API filters](https://abs.twimg.com/responsive-web/client-web/bundle.LoggedInApiFilters.31c5a6f3d43634eea.js),
  SHA-256 `91add1aa282df4773696c495afbba785a3671c1ea5e8129a5a607b1d31e6725c`,
  contain no SearchTimeline GET-to-POST rewrite. The inspected session-binding
  operation allowlist does not include SearchTimeline; its presence elsewhere
  is not evidence that search requires those signing operations.

The query ID and GET-only routing are unchanged. No old-ID fallback, POST probe,
transaction-ID fabrication, other-account attempt or public-content fallback
was added. HTTP 404 alone does not establish whether an operation was removed,
a request contract was rejected, or account/client policy obscured the endpoint.
A consented local retry is needed to assess the corrected header. If 404 remains,
compare only the successful browser request's method and operation path (not
cookies, headers, query variables, a HAR, or a copied cURL command) before changing
transport or IDs.

### Phase 2 bookmark-read evidence — 2026-09-12

Public `/explore` now advertises
[main.ef8e0e0fdc2bb9c0a.js](https://abs.twimg.com/responsive-web/client-web/main.ef8e0e0fdc2bb9c0a.js),
1,125,707 bytes, SHA-256
`290a24f210c6b7310e3abe0abbf935a748e6b48719db6c9bf30782b44c206e59`.
All six existing query IDs and complete operation metadata match the old main.
The original pinned main remains available with its recorded hash; the runtime
public authorization-value source and existing operations are unchanged.

Bookmarks is defined separately in
[shared~bundle.BookmarkFolders~bundle.Bookmarks.2be1bbc341bba456a.js](https://abs.twimg.com/responsive-web/client-web/shared~bundle.BookmarkFolders~bundle.Bookmarks.2be1bbc341bba456a.js),
95,436 bytes, SHA-256
`c1fc402b73ee04bf5d03964285e0bbd0939cf4916f11cd3a227293fd59e84a33`.
Module 842483 declares `queryId:"tF6KOjmZM0WGcB2Q0mfwhw"`,
`operationName:"Bookmarks"`, `operationType:"query"`. This definition is not
present in the old pinned main and is not attributed to it. The auxiliary source
was inspected statically, not executed or downloaded at runtime.

Module 426820 imports that definition and calls:

```js
fetchBookmarksTimeline:({count:r,cursor:s})=>
  e.graphQL(p(),{count:r,cursor:s,includePromotedContent:!0,...(0,i.g)(t)},
    (e,t)=>!t?.bookmark_timeline_v2?.timeline)
    .then(e=>e?.bookmark_timeline_v2?.timeline||k.yB)
```

The shared helper spreads an empty export (module 812055) in
[vendor.b78f62166e835d11a.js](https://abs.twimg.com/responsive-web/client-web/vendor.b78f62166e835d11a.js),
SHA-256 `ba857ea02535f311863f0b092c9d9a96ed76a3956496a0135b1f3d1428455cdd`.
The query has no `forcePost`; the normal adapter uses GET with JSON variables,
features, and `content-type: application/json`. The implemented variable shape
is `{count, cursor?, includePromotedContent:true}`: no user ID, voice flag,
folder selector, or call-site field toggles. The request omits the `fieldToggles`
parameter (corrected in v0.5.1 after direct adapter reinspection). Its declared 39 feature names exactly match post set P,
and its eight declared toggles match the other collection operations. Reusing
feature membership is source-supported; authenticated values remain unverified.

The raw response root is
`/data/bookmark_timeline_v2/timeline/instructions`. The browser adapter removes
`data` before the quoted continuation. Unlike the browser's empty-state fallback,
xcli treats a missing root as a protocol error. The Bookmarks timeline uses
formatter 225219, which consumes `instructions`, including add/replace entries
and modules. Cursor parser 669251 reads `cursorType`/`value`; shared cursor logic
recognizes `Bottom`. These support the existing bounded parser, not a claim of
complete schemas or exhaustive bookmarks.

Only public source was fetched. No credentials, guest activation, authenticated
requests, or live mutation tests were used. `Bookmarks` is a read query; folder,
add, and remove operations are excluded. Live interoperability, account-specific
feature values and server count behavior remain unverified. The separately
observed search HTTP 404 is not resolved by this research.

### Phase 2 own-liked-post evidence — 2026-09-12

Current main `main.ef8e0e0fdc2bb9c0a.js` (hash recorded above), module 837876,
declares `queryId:"o000A_Cp4JPOihhbeEgi0g"`, `operationName:"Likes"`,
`operationType:"query"`. Its 39 feature names exactly match P and the independent
bookmark feature fixture; all eight declared post toggles match as well. This
source review does not replace the existing pinned runtime authorization asset.

Module 965808 imports that operation and calls:

```js
fetchLikes:({count:r,cursor:i,userId:o})=>e.graphQL(a(),{
  userId:o,count:r,cursor:i,includePromotedContent:!1,...(0,n.g)(t),
  withClientEventToken:!1,withBirdwatchNotes:!1,
  withVoice:t.isTrue("voice_consumption_enabled")
},M,z(t))
```

`z(t)` supplies `fieldToggles:{withArticlePlainText:false}` (the browser's
expression is a feature flag AND false). The shared helper again resolves to
empty vendor module 812055. No `forcePost` is supplied, so the reviewed query
uses GET and the normal JSON/content-type envelope. xcli sends `withVoice:false`
as an explicit conservative optional-variable choice; authenticated feature
values are not established by these public sources.

The continuation requires `user.result.__typename === "User"`, then selects
`user.result.timeline.timeline`. xcli requires that discriminator and the raw
root `/data/user/result/timeline/timeline/instructions`; it does not reproduce
the browser's empty-state fallback for missing data. Unknown/unavailable user
discriminators fail as protocol errors, never empty successes.

[UserProfile bundle](https://abs.twimg.com/responsive-web/client-web/bundle.UserProfile.5de779fab7d9982aa.js),
375,047 bytes, SHA-256
`a7292f22e102ddf4213ef8664422d2b09202719ae2016338f22d94b652c972c4`,
module 784820 passes `{count,userId,cursor?}` to `fetchLikes` and uses formatter
225219 with context `FETCH_LIKES_TIMELINE`. The formatter/cursor handling supports
the same add/replace/module instructions and opaque Bottom cursor described
above. Exhaustiveness and actual server count limits remain unverified.

The profile UI computes `Y = viewerUserId === profile.id_str`; it includes the
Likes route only when `Y` is true. xcli therefore derives `userId` exclusively
from the explicit account's verified Viewer identity. There is no target-user
argument or arbitrary-user Likes fallback. This is an application boundary,
not evidence of server-enforced authorization behavior.

**Rollout caveat:** when `responsive_web_history_screen_enabled` is enabled,
the same own-profile route redirects to `/i/history/likes`. The fetched logged-out
configuration enables this flag. The Likes query and direct caller remain in the
bundle, but may not be the active path for every authenticated rollout. No History
endpoint was researched or added as a fallback. This is experimental static
support, not successful live interoperability. All fetches were anonymous public
assets; no browser/Keychain access, authenticated calls, guest activation or
mutations occurred.

### Phase 2 latest list-post evidence — 2026-09-12

The reviewed latest operation is `ListLatestTweetsTimeline`, query ID
`u6PUF1835XGBkf6MQZUV8A`, type `query`, in module 678316 of
[shared chunk 25406](https://abs.twimg.com/responsive-web/client-web/shared~loader.Dock~bundle.BookmarkFolders~bundle.Bookmarks~bundle.Explore~bundle.HomeTimeline~bundle.Notifica.dd20d1d8c1f4a4bca.js),
152,713 bytes, SHA-256
`c208d767c6aacb1ce87e30cfe7df7b3cc0d80471fc15d465b0298a59751b9a50`.
Current main/vendor match the hashes recorded above. Anonymous `/explore`
redirected to logged-out login HTML; its static webpack map resolved the chunks.

Module 776254 imports the latest definition as `P` and calls:

```js
fetchTweetsGraphQL(n,i){
  let{count:r,cursor:a,listId:s,useRanked:l}=n,
      d=l?K():P(),_={listId:s,count:r,cursor:a,...(0,o.g)(t)};
  return e.graphQL(d,_,ey)
    .then(e=>e.list.tweets_timeline?.timeline||ei.yB)
}
```

The selected latest branch corresponds to client-only `useRanked:false`; that
flag is not transmitted and no ranked operation/fallback is implemented. The
shared-variable helper is again the empty vendor export. Transmitted variables
are exactly `{listId,count,cursor?}`. No user ID, promoted-content flag or voice
flag is supplied. The call has no force-POST options or field toggles: the
adapter uses GET and omits the `fieldToggles` parameter (corrected in v0.5.1). The 39 declared feature names
exactly match P (including order), verified against the independent bookmark
fixture; authenticated boolean values remain unverified.

[UserLists bundle](https://abs.twimg.com/responsive-web/client-web/bundle.UserLists.295cb90da11794e5a.js),
105,316 bytes, SHA-256
`5f7c8bc573263490bedfedcdb4809c8aa3dba85ea2680b9c235c95d723731bfb`,
module 286666 defaults `useRanked` to false, supplies `{listId,count,cursor?}`
plus the client-only mode selector, and uses formatter 225219 with context
`FETCH_LIST_TIMELINE_GRAPHQL`. The validation predicate requires
`list.tweets_timeline.timeline`; the raw root is
`/data/list/tweets_timeline/timeline/instructions`. No top-level `List` typename
requirement was observed. xcli requires the root, not a fabricated discriminator
or the browser's empty-state fallback.

[Current API filters](https://abs.twimg.com/responsive-web/client-web/bundle.LoggedInApiFilters.a9f8af9d64457a3aa.js),
65,328 bytes, SHA-256
`4605ee073ae503c1c034762d8db6dfcd61f49223d173cc04cb00ecbbf851754f`,
include this operation in a GET-only prefetch-reuse allowlist. It was absent
from the inspected session-binding operation list; no GET-to-POST rewrite was
found. This does not establish all live authentication requirements.

The shared formatter supports add/replace/module/pin/terminate instructions and
opaque Bottom cursors. xcli retains its strict parser: normal Tweet and wrapped
Tweet results are normalized, unavailable/tombstone results are not ordinary
complete posts, and unsupported `TweetPreviewDisplay` results fail rather than
masquerading as full content. Missing roots are errors. Cursor exhaustion never
proves an exhaustive list archive.

All research was bounded, anonymous public static-asset inspection. No GraphQL
requests, JS execution, credentials, guest activation or mutations were used.
Private-list permissions, account-specific features, inaccessible-list responses
and live pagination/interoperability remain unverified. List discovery, metadata,
ranked timelines and membership/write operations are outside this increment.

### Phase 2 relationship-read evidence — 2026-09-12

Current main (hash recorded above) defines two GET queries: module 747136,
`Following` / `4EQGMEhtdVw8NeVBDQHESQ`, and module 810724,
`Followers` / `sF7aRC2fRq7OGOOp_qHntA`, both `operationType:"query"`.
Module 175350 calls them with `{userId,count,cursor?,includePromotedContent:false,
withGrokTranslatedBio:<followers bio translation feature>}` and the empty shared
helper. There is no force-POST option. Both complete ordered feature arrays equal
the 39-name post set P, not the smaller USER set. Their eight declared toggles
match post metadata, but these callers omit `fieldToggles`; xcli omits that URL
parameter for these operations rather than sending generic post toggles. Bio
translation is conservatively false; public logged-out true observations are not
claimed as authenticated rollout values.

The continuation requires `user.result.__typename === "User"`, then selects
`user.result.timeline.timeline`. Both raw roots are
`/data/user/result/timeline/timeline/instructions`. Missing/unknown owner data
fails rather than reproducing the browser's empty fallback.

[UserFollowLists bundle](https://abs.twimg.com/responsive-web/client-web/bundle.UserFollowLists.22e2b2e9247fbdf7a.js),
36,628 bytes, SHA-256
`b522739e1a772830d3c1d5cf40591f6812c3328404770378f5f1ceca1fca8768`,
uses formatter 225219 and passes `{count,userId,cursor?}`. Item formatter 357725
recognizes **`itemType:"TimelineUser"`**, then module 60850 reads
`user_results.result`, requires `__typename:"User"` and `core`. Module 366829
uses `rest_id` as the ID and `core.screen_name` as the handle. xcli's user
projection therefore requires those fields; it does not apply the legacy
post-author fallback. Flat items and module items are supported, as are
add/replace/pin/add-to-module instructions and opaque Bottom cursors. Unsupported
removal instructions and unavailable/unknown user variants currently fail closed,
not as empty successes. Returned users are deduplicated by stable ID.

The browser can supply another profile's ID. xcli's own-account-only selection is
a conservative product boundary, **not a source/server restriction**: following
and followers commands derive `userId` only from the explicit account's verified
Viewer. No arbitrary target user, account rotation or endpoint fallback exists.

This was anonymous bounded static inspection using the current main/vendor hashes
already recorded above, with no credentials, JS execution, GraphQL requests or
mutations. Live availability, authentication/transaction requirements, account
features, server permissions and complete pagination variants remain unverified.
Neither cursor exhaustion nor a returned count proves an exhaustive social graph.

### Correction: absent field-toggle options — 2026-09-12

Direct reinspection of the hashed current main adapter found:

```js
let p=function(e,t){if(e&&e.length>0&&t){let r={};
  return e.forEach(e=>{t?.hasOwnProperty(e)&&(r[e]=t?.[e])}),r}}
  (e?.fieldToggles,_?.fieldToggles);
// GET request construction:
t&&(u.fieldToggles=JSON.stringify(t))
```

Without caller options, the filtered toggle value is undefined, so the URL
parameter is omitted. Earlier bookmark/list research summaries incorrectly
inferred an empty object from declared metadata. v0.2.0–v0.5.0 sent `{}` for
Bookmarks, and v0.4.0–v0.5.0 did so for ListLatestTweetsTimeline. v0.5.1 omits
those parameters, with exact request regression tests. Following/Followers
already omit them; Likes retains its explicit `withArticlePlainText:false`.
This fixes an established source-contract discrepancy, not a proven cause of any
live failure. Query IDs, features, methods and credential boundaries are unchanged.

### List-member reads — 2026-09-12

The same hash-verified shared chunk 25406 used for latest-list posts
(`c208d767c6aacb1ce87e30cfe7df7b3cc0d80471fc15d465b0298a59751b9a50`)
defines module 488153: `ListMembers`, ID `ljlktihgwXeYTfHwwiPj5A`, type `query`.
Module 776254 binds `O=n(488153),S=n.n(O)` and calls:

```js
fetchMembersGraphQL(n,i){let{count:r,cursor:a,listId:s}=n;
  return e.graphQL(S(),{listId:s,count:r,cursor:a,...(0,o.g)(t)},ew)
    .then(e=>e.list.members_timeline?.timeline||ei.yB)}
```

The shared helper is empty; `ew` rejects a missing
`list.members_timeline.timeline`. The raw root is
`/data/list/members_timeline/timeline/instructions`, with no invented owner
User/List discriminator. The caller's timeline factory uses formatter 225219,
context `FETCH_MEMBERS`, and parameters `{count,cursor?,listId}`; only string
cursors pass through. Normal user items therefore use the same reviewed
`TimelineUser`/`user_results.result` projection documented above.

The 39 feature names match the independent post-set fixture exactly. No fourth
options argument or force-POST option is supplied: GET with no `fieldToggles`
parameter, following the corrected adapter evidence above. No user ID, ranked,
voice or promoted-content variable is transmitted. xcli requires an explicit
account and validated list ID; its cache separates member views from list-post
views and other accounts. Unknown/unavailable items fail closed. No membership
changes or claims of exhaustive membership are included. Source checks were
anonymous static reads; live list permissions and interoperability are unverified.

## 3. macOS Chrome credential access: supported boundary

First-party source snapshot: Chromium **`233e625e16284f1f1e11150b88ea16e43c325c37`**, inspected 2026-09-10. This is mainline source, **not a verified installed/released Chrome build**:

- [C-key-provider](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/components/os_crypt/async/browser/keychain_key_provider.mm)
- [C-password](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/components/os_crypt/common/keychain_password_mac.mm), [C-Keychain API](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/crypto/apple/keychain_v2.mm)
- [C-encryptor](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/components/os_crypt/async/common/encryptor.cc)
- [C-cookie-store](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/net/extras/sqlite/sqlite_persistent_cookie_store.cc)
- [C-profile directories](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/docs/user_data_dir.md)
- Historical corroboration: [macOS sync crypto at tag `143.0.7497.1`](https://github.com/chromium/chromium/blob/143.0.7497.1/components/os_crypt/sync/os_crypt_mac.mm). That sync path is absent on inspected main; use the async sources above for current claims.

Recommended initial support contract (design, not tested extraction):

1. **Explicit profile opt-in.** Chrome Stable's documented macOS user-data root is `~/Library/Application Support/Google/Chrome`; profiles are separate children such as `Default`/`Profile 1`. Use explicit selection and safe metadata-only discovery. Verify the selected build's cookie location/schema; do not assume a Windows `Network/Cookies` layout applies to every macOS version. No real profile was inspected here.
2. **OS-supported read, not protection bypass.** Use Security.framework `SecItemCopyMatching` for the existing generic password, service **`Chrome Safe Storage`**, account **`Chrome`**, as corroborated by C-password/C-Keychain API. Chromium uses `Chromium Safe Storage`/`Chromium`, a different brand. Respect system authorization prompts and denial/locked-Keychain errors. Never create/reset the browser's Keychain item: Chromium itself may create one on “not found,” but this read-only tool must not copy that behavior. Native in-process access avoids a shell command printing key material.
3. **Restrict DB access.** Read only the selected cookie database with a consistent read-only SQLite transaction/backup strategy that includes WAL state; copying just a live main DB can miss current cookies. Prefer an in-memory snapshot and no complete cookie dump. If locked/inconsistent, ask the user to close Chrome; do not kill it or change database permissions. Restrict any unavoidable temporary files, remove them reliably, and never use `immutable=1` on a changing live DB as a consistency shortcut.
4. **Select minimally.** Only `auth_token`/`ct0` applicable to the exact X request host/path, from the same selected profile and coherent domain/session. Respect expiry, secure flag, host-only versus domain cookies, path and partition context; reject ambiguity. Do not match arbitrary suffixes such as `evilx.com`, merge X and legacy Twitter jars blindly, or treat HttpOnly as “not needed.” Reject unsupported partitioned-cookie cases rather than silently selecting them. Do not infer independent sessions from X's in-profile account switcher.
5. **Verified `v10` decoding.** C-key-provider derives **16 bytes** using PBKDF2-HMAC-SHA1 over the Keychain password bytes, salt `saltysalt`, **1003 iterations**. Strip the three-byte ASCII `v10` provider tag, then AES-128-CBC with IV **16 ASCII spaces** and strict PKCS#7 padding validation. The stored password's textual bytes are the KDF input; do not base64-decode it merely because Chrome generated base64-looking text.
6. **Host digest is mandatory for schema 24.** Inspect `meta.version` and compatibility/schema before decoding. In schema 24, encrypted plaintext is **`SHA256(host_key exact stored bytes) || cookie_value`**. Compare the first **32 raw bytes**, including the leading dot in the hashed domain when stored, then strip only on successful verification. Do not hex-encode the digest, hash the cookie name, blindly drop 32 bytes, or decode the binary digest as UTF-8. C-cookie-store currently declares version/compatible-version 24 and rejects digest mismatch. Earlier schemas need explicitly versioned fixtures; unknown future schemas fail closed. If both plaintext `value` and `encrypted_value` are nonempty, current Chromium rejects the row. Plaintext storage, if intentionally supported, must not be subjected to encrypted-value digest stripping.
7. **Protected/unknown formats are blockers.** The inspected Windows [app-bound provider header](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/chrome/browser/os_crypt/app_bound_encryption_provider_win.h) defines data tag `v20` and wrapped-key marker `APPB`; [implementation](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/chrome/browser/os_crypt/app_bound_encryption_provider_win.cc) uses AES-256-GCM and OS app-bound services. This is **not** macOS `v10`, and prefix alone is not a cross-platform decoding specification. Reject `v20`, other unknown tags, unavailable/non-exportable keys, and unauthorized access for initial macOS support. No process injection, elevated bypass, browser remote-debugging workaround, security-policy downgrade, mock/empty-password fallback, or key-generation fallback. C-encryptor has a compatibility empty-key fallback; deliberately exclude it from this support contract.
8. Keep both session cookies and the much broader Safe Storage key out of logs, model/tool transcripts, environment variables, process arguments, normal output and third-party requests. Minimize lifetime and zeroize practical buffers. Persist only profile reference/expected stable identity; if secrets must persist, use a dedicated OS credential-store item. Decryption success proves storage compatibility, **not authenticated identity**: verify Viewer before registering/reusing the connection and reject silent ID changes.

## 4. Implementation recommendations and remaining blockers

1. Proceed with unauthenticated FxTwitter reads separately. Never send X session headers to that backend or silently substitute it for an explicitly account-selected request.
2. Keep a small reviewed **allowlist of `(operationName, queryId, operationType=query, method, feature names, toggles, source URL/hash)`**. Discover/parse adjacent object definitions without evaluating remote JS. Reject duplicates/conflicts and any mutation; IDs alone or regexes spanning unrelated modules are unsafe. Never blindly recycle Bird's fallback IDs. A refresh may occur once for convincing manifest mismatch, not on every 401/403/429.
3. Initial GraphQL transport should be GET-only for these observed queries, strict HTTPS X-host allowlist, no cross-origin credential redirects, bounded request/response sizes, account-isolated cache/rate-limit state, and redacted typed errors. Read-only is an application allowlist, not a limitation on the stolen/reused session's power.
4. **Not yet established:** live Viewer identity semantics for a consenting account; exact required authenticated headers/transaction behavior; released-Chrome profile database path/build support; feature values for authenticated rollouts; complete response schemas and pagination branches. No claim that the IDs above currently execute successfully is justified without those tests.
5. Before shipping auth: opt-in local tests with user-controlled accounts, sanitized fixtures for identity mismatch, wrapped/unavailable posts, note text, mixed GraphQL errors, cursor replacement/repetition, expiry, 401/403/429, operation churn and partial pages. Storage tests should be synthetic: schema 23/24, exact-domain digest mismatch, bad padding, malformed/unknown prefixes, conflicting rows, partition ambiguity, Keychain denial and SQLite WAL. Do not send raw fixtures containing cookies or private account data to an agent.
6. Stop on identity uncertainty, unsupported protection/schema, consent denial, rate limits or missing authoritative definitions; explain the blocker rather than trying another profile/account, bypassing protections, or claiming completeness. Unofficial endpoints remain subject to X policy/enforcement and can break without notice.
