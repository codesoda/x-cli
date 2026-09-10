//! Source-verified read-query snapshot; see docs/protocol-research.md for provenance and limitations.
use serde_json::{Value, json};

pub const BUNDLE_URL: &str =
    "https://abs.twimg.com/responsive-web/client-web/main.7313c8670b523249a.js";
pub const BUNDLE_SHA256: &str = "f2c17811723ec0617ef6cf213fd56eb732d4f042c7ff81b7d72e33778f9ead65";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Viewer,
    Post,
    Detail,
    Search,
    User,
    Timeline,
}
impl Operation {
    pub fn name(self) -> &'static str {
        match self {
            Self::Viewer => "Viewer",
            Self::Post => "TweetResultByRestId",
            Self::Detail => "TweetDetail",
            Self::Search => "SearchTimeline",
            Self::User => "UserByScreenName",
            Self::Timeline => "UserTweets",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Viewer => "9t128XgFic52jPUEkJMf6w",
            Self::Post => "snmujSvB_9WXyd8yjvZ24Q",
            Self::Detail => "FyR-GrebyjdkRoW1z6uCgQ",
            Self::Search => "KPSo2_UWdOMpPJwjhfT1Qg",
            Self::User => "KybxDj9RrADIITXlGG8kpw",
            Self::Timeline => "OeFjWKHutsuyWXZGmLr02A",
        }
    }
    pub fn root(self) -> &'static str {
        match self {
            Self::Viewer => "/data/viewer/user_results/result",
            Self::Post => "/data/tweetResult/result",
            Self::Detail => "/data/threaded_conversation_with_injections_v2/instructions",
            Self::Search => "/data/search_by_raw_query/search_timeline/timeline/instructions",
            Self::User => "/data/user/result",
            Self::Timeline => "/data/user/result/timeline/timeline/instructions",
        }
    }
    pub fn features(self) -> Value {
        let names = match self {
            Self::Viewer => VIEWER,
            Self::User => USER,
            _ => POST,
        };
        let mut map = serde_json::Map::new();
        for name in names.split_whitespace() {
            if self == Self::Post
                && matches!(
                    name,
                    "rweb_video_screen_enabled" | "responsive_web_enhance_cards_enabled"
                )
            {
                continue;
            }
            // Source-observed logged-out snapshot, NOT claimed authenticated rollout values.
            let enabled = !DISABLED.split_whitespace().any(|n| n == name);
            map.insert(name.into(), json!(enabled));
        }
        Value::Object(map)
    }
    pub fn toggles(self) -> Value {
        if self == Self::Viewer {
            json!({"isDelegate":false,"withAuxiliaryUserLabels":false,"withPayments":false})
        } else {
            json!({"withAuxiliaryUserLabels":false,"withPayments":false})
        }
    }
}
const VIEWER: &str = "subscriptions_upsells_api_enabled profile_label_improvements_pcf_label_in_post_enabled responsive_web_profile_redirect_enabled rweb_tipjar_consumption_enabled verified_phone_label_enabled creator_subscriptions_tweet_preview_api_enabled responsive_web_graphql_timeline_navigation_enabled";
const USER: &str = "hidden_profile_subscriptions_enabled profile_label_improvements_pcf_label_in_post_enabled responsive_web_profile_redirect_enabled rweb_tipjar_consumption_enabled verified_phone_label_enabled subscriptions_verification_info_is_identity_verified_enabled subscriptions_verification_info_verified_since_enabled highlights_tweets_tab_ui_enabled responsive_web_twitter_article_notes_tab_enabled subscriptions_feature_can_gift_premium creator_subscriptions_tweet_preview_api_enabled responsive_web_graphql_timeline_navigation_enabled";
const POST: &str = "rweb_video_screen_enabled rweb_cashtags_enabled profile_label_improvements_pcf_label_in_post_enabled responsive_web_profile_redirect_enabled rweb_tipjar_consumption_enabled verified_phone_label_enabled creator_subscriptions_tweet_preview_api_enabled responsive_web_graphql_timeline_navigation_enabled premium_content_api_read_enabled communities_web_enable_tweet_community_results_fetch c9s_tweet_anatomy_moderator_badge_enabled responsive_web_grok_analyze_button_fetch_trends_enabled responsive_web_grok_analyze_post_followups_enabled rweb_cashtags_composer_attachment_enabled responsive_web_jetfuel_frame rweb_sports_post_context_enabled responsive_web_grok_share_attachment_enabled responsive_web_grok_annotations_enabled articles_preview_enabled responsive_web_edit_tweet_api_enabled rweb_conversational_replies_downvote_enabled graphql_is_translatable_rweb_tweet_is_translatable_enabled view_counts_everywhere_api_enabled longform_notetweets_consumption_enabled responsive_web_twitter_article_tweet_consumption_enabled content_disclosure_indicator_enabled content_disclosure_ai_generated_indicator_enabled responsive_web_grok_show_grok_translated_post responsive_web_grok_analysis_button_from_backend post_ctas_fetch_enabled freedom_of_speech_not_reach_fetch_enabled standardized_nudges_misinfo tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled longform_notetweets_rich_text_read_enabled longform_notetweets_inline_media_enabled responsive_web_grok_image_annotation_enabled responsive_web_grok_imagine_annotation_enabled responsive_web_grok_community_note_auto_translation_is_enabled responsive_web_enhance_cards_enabled";
// Last two are explicit conservative choices for missing/conflicting public configuration.
const DISABLED: &str = "rweb_video_screen_enabled rweb_tipjar_consumption_enabled verified_phone_label_enabled premium_content_api_read_enabled responsive_web_grok_analyze_button_fetch_trends_enabled responsive_web_grok_analyze_post_followups_enabled rweb_sports_post_context_enabled post_ctas_fetch_enabled longform_notetweets_inline_media_enabled responsive_web_enhance_cards_enabled subscriptions_upsells_api_enabled rweb_conversational_replies_downvote_enabled subscriptions_feature_can_gift_premium";
