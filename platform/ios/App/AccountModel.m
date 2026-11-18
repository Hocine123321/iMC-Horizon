#import "AccountModel.h"

NSString *const HZDoesNotOwn = @"This account doesn't own Minecraft: Java Edition.";
NSString *const HZNoProfile =
    @"This account has no Minecraft profile yet. Create one at minecraft.net, then sign in again.";
NSString *const HZSignInAgain = @"Your Microsoft sign-in expired. Sign in again.";
NSString *const HZAppNotApproved =
    @"The Minecraft services refused this sign-in. The app ID it uses may be blocked right now. Try again later, or set a different app ID in Settings > Developer options.";

// The legacy Microsoft app ID of the official launcher family, which most launchers sign in with.
NSString *const HZDefaultClientID = @"00000000402b5328";

// A client ID is letters, digits and dashes only, 8 to 64 long. Anything else is not an ID, so it is not used.
static NSString *CleanClientID(NSString *raw) {
    if (![raw isKindOfClass:NSString.class]) return nil;
    NSString *id_ = [raw stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
    NSCharacterSet *bad = [[NSCharacterSet characterSetWithCharactersInString:
        @"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-"] invertedSet];
    if (id_.length < 8 || id_.length > 64 || [id_ rangeOfCharacterFromSet:bad].location != NSNotFound) return nil;
    return id_;
}

static NSString *IDFile(NSString *name) {
    NSString *docs = NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES).firstObject;
    return [[docs stringByAppendingPathComponent:@"Horizon"] stringByAppendingPathComponent:name];
}

// Only a short file counts: an ID is 16 to 36 characters, so a stray large file is not read whole.
static NSString *ReadIDFile(NSString *name) {
    NSString *file = IDFile(name);
    NSDictionary *attrs = [NSFileManager.defaultManager attributesOfItemAtPath:file error:nil];
    if (!attrs || [attrs fileSize] == 0 || [attrs fileSize] > 256) return nil;
    return [NSString stringWithContentsOfFile:file encoding:NSUTF8StringEncoding error:nil];
}

static NSString *ReadCustomClientID(void) {
    // Never baked into a build. Three places, the first that holds a valid ID wins:
    //  1. the environment variable HORIZON_MSA_CLIENT_ID (a desktop run, or a launcher that sets one);
    //  2. Documents/Horizon/app-id.txt, written by Settings > Developer options or put there with the Files app;
    //  3. Documents/Horizon/test-client-id.txt, the name the first version used.
    NSString *id_ = CleanClientID(NSProcessInfo.processInfo.environment[@"HORIZON_MSA_CLIENT_ID"]);
    if (!id_) id_ = CleanClientID(ReadIDFile(@"app-id.txt"));
    if (!id_) id_ = CleanClientID(ReadIDFile(@"test-client-id.txt"));
    return id_;
}

// Read once, and again only when the player sets or clears it in the app: the ID must not change under a
// sign-in that has started, and the file is not read again for every request.
static NSString *sCachedID;
static BOOL sCachedIDLoaded;
static NSObject *IDLock(void) {
    static NSObject *lock; static dispatch_once_t once;
    dispatch_once(&once, ^{ lock = [NSObject new]; });
    return lock;
}

NSString *HZCustomClientID(void) {
    @synchronized (IDLock()) {
        if (!sCachedIDLoaded) { sCachedID = ReadCustomClientID(); sCachedIDLoaded = YES; }
        return sCachedID;
    }
}

BOOL HZSetCustomClientID(NSString *value, NSString **problem) {
    NSString *file = IDFile(@"app-id.txt");
    NSString *trimmed = [value stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
    @synchronized (IDLock()) {
        if (trimmed.length == 0) {
            [NSFileManager.defaultManager removeItemAtPath:file error:nil];
            [NSFileManager.defaultManager removeItemAtPath:IDFile(@"test-client-id.txt") error:nil];
        } else {
            NSString *clean = CleanClientID(trimmed);
            if (!clean) {
                if (problem) *problem = @"That is not an app ID. It is letters, digits and dashes only, 8 to 64 characters.";
                return NO;
            }
            [NSFileManager.defaultManager createDirectoryAtPath:[file stringByDeletingLastPathComponent]
                                    withIntermediateDirectories:YES attributes:nil error:nil];
            NSError *err = nil;
            if (![clean writeToFile:file atomically:YES encoding:NSUTF8StringEncoding error:&err]) {
                if (problem) *problem = @"The ID could not be saved on this phone.";
                return NO;
            }
            // An older file under the first version's name must not win back over the new one.
            [NSFileManager.defaultManager removeItemAtPath:IDFile(@"test-client-id.txt") error:nil];
        }
        sCachedIDLoaded = NO;   // read again on the next ask
    }
    return YES;
}

NSString *HZActiveClientID(void) { return HZCustomClientID() ?: HZDefaultClientID; }

// The reply as a dictionary, or nil when it is not a JSON object.
static NSDictionary *Object(NSData *body) {
    if (body.length == 0) return nil;
    id v = [NSJSONSerialization JSONObjectWithData:body options:0 error:nil];
    return [v isKindOfClass:NSDictionary.class] ? v : nil;
}

static NSString *Str(NSDictionary *d, NSString *key) {
    id v = d[key];
    return [v isKindOfClass:NSString.class] && [v length] > 0 ? v : nil;
}

// JSON numbers arrive as NSNumber; the services also send some as strings. nil when neither.
static NSNumber *Num(NSDictionary *d, NSString *key) {
    id v = d[key];
    if ([v isKindOfClass:NSNumber.class]) return v;
    if ([v isKindOfClass:NSString.class]) {
        NSScanner *s = [NSScanner scannerWithString:v];
        long long n;
        if ([s scanLongLong:&n] && s.atEnd) return @(n);
    }
    return nil;
}

// The OAuth error codes as messages for the player (the desktop launcher's words).
static NSString *OAuthError(NSDictionary *v) {
    NSString *e = Str(v, @"error");
    if ([e isEqualToString:@"authorization_declined"]) return @"Sign-in was declined in the browser.";
    if ([e isEqualToString:@"expired_token"]) return @"The sign-in code expired. Start again.";
    if ([e isEqualToString:@"bad_verification_code"]) return @"The sign-in code was not recognised. Start again.";
    if ([e isEqualToString:@"invalid_grant"]) return HZSignInAgain;
    if ([e isEqualToString:@"invalid_client"] || [e isEqualToString:@"unauthorized_client"])
        return @"Microsoft refused this app's registration (client ID).";
    if (e) return [NSString stringWithFormat:@"Microsoft sign-in failed (%@).", e];
    return @"Microsoft sign-in failed.";
}

@implementation HZDeviceCode
+ (instancetype)parse:(NSData *)body error:(NSString **)error {
    NSDictionary *v = Object(body);
    NSString *user = Str(v, @"user_code"), *device = Str(v, @"device_code");
    if (!user || !device) {
        if (error) *error = v ? OAuthError(v) : @"Microsoft sign-in failed.";
        return nil;
    }
    HZDeviceCode *c = [HZDeviceCode new];
    c.userCode = user;
    c.deviceCode = device;
    c.verificationURI = Str(v, @"verification_uri") ?: @"https://www.microsoft.com/link";
    NSNumber *interval = Num(v, @"interval");
    c.interval = interval ? MAX(1, interval.doubleValue) : 5;   // never poll faster than once a second
    NSNumber *expires = Num(v, @"expires_in");
    c.expiresIn = expires ? expires.doubleValue : 900;
    return c;
}
@end

@implementation HZPollOutcome
+ (instancetype)parse:(NSData *)body status:(NSInteger)status {
    HZPollOutcome *o = [HZPollOutcome new];
    NSDictionary *v = Object(body);
    NSString *e = Str(v, @"error");
    if ([e isEqualToString:@"authorization_pending"]) { o.result = HZPollPending; return o; }
    if ([e isEqualToString:@"slow_down"]) { o.result = HZPollSlowDown; return o; }
    NSString *access = Str(v, @"access_token"), *refresh = Str(v, @"refresh_token");
    if (status == 200 && access && refresh) {
        o.result = HZPollSignedIn;
        o.accessToken = access;
        o.refreshToken = refresh;
        return o;
    }
    o.result = HZPollStopped;
    o.message = v ? OAuthError(v) : @"Microsoft sign-in failed.";
    return o;
}
@end

NSString *HZXstsMessage(long long xerr) {
    switch (xerr) {
        case 2148916233: return @"This Microsoft account has no Xbox profile yet. Sign in once at xbox.com to create one, then try again.";
        case 2148916235: return @"Xbox Live is not available in your country or region.";
        case 2148916236:
        case 2148916237: return @"This account needs adult verification on xbox.com before it can play.";
        case 2148916238: return @"This is a child account: an adult has to add it to a Microsoft family first.";
        default: return nil;
    }
}

@implementation HZXstsReply
+ (instancetype)parse:(NSData *)body status:(NSInteger)status {
    HZXstsReply *r = [HZXstsReply new];
    NSDictionary *v = Object(body);
    NSString *token = Str(v, @"Token");
    NSArray *xui = [v[@"DisplayClaims"] isKindOfClass:NSDictionary.class] ? v[@"DisplayClaims"][@"xui"] : nil;
    NSDictionary *first = [xui isKindOfClass:NSArray.class] && xui.count > 0 && [xui[0] isKindOfClass:NSDictionary.class] ? xui[0] : nil;
    NSString *uhs = Str(first, @"uhs");
    if (status == 200 && token && uhs) {
        r.token = token;
        r.userHash = uhs;
        return r;
    }
    NSNumber *xerr = Num(v, @"XErr");
    if (xerr) {
        r.message = HZXstsMessage(xerr.longLongValue) ?:
            [NSString stringWithFormat:@"Xbox Live refused the sign-in (error %lld).", xerr.longLongValue];
    } else {
        r.message = @"Xbox Live refused the sign-in.";
    }
    return r;
}
@end

NSString *HZMinecraftIdentityToken(NSString *userHash, NSString *xstsToken) {
    return [NSString stringWithFormat:@"XBL3.0 x=%@;%@", userHash, xstsToken];
}

BOOL HZOwnsJavaEdition(NSData *body) {
    NSArray *items = Object(body)[@"items"];
    if (![items isKindOfClass:NSArray.class]) return NO;
    for (id item in items) {
        if (![item isKindOfClass:NSDictionary.class]) continue;
        NSString *name = Str(item, @"name");
        if ([name isEqualToString:@"game_minecraft"] || [name isEqualToString:@"product_minecraft"]) return YES;
    }
    return NO;
}

@implementation HZProfile
+ (instancetype)parse:(NSData *)body status:(NSInteger)status message:(NSString **)message {
    if (status == 404) {
        if (message) *message = HZNoProfile;
        return nil;
    }
    NSDictionary *v = Object(body);
    NSString *name = Str(v, @"name"), *uuid = Str(v, @"id");
    if (status != 200 || !name || !uuid) {
        if (message) *message = @"Could not read this account's Minecraft profile.";
        return nil;
    }
    HZProfile *p = [HZProfile new];
    p.name = name;
    p.uuid = uuid;
    return p;
}
@end

@implementation HZAccountState
@end

BOOL HZMayDownload(BOOL signedIn, BOOL owns, BOOL reachable, BOOL freshAnswer) {
    if (!signedIn) return NO;
    if (reachable) return freshAnswer;   // the services' answer now, yes or no
    return owns;                         // offline: the last answer stands
}
