// Valkyrie model adapter. Uses only the MIT-licensed Purism Core C API.
// The spring solver below is independent code; it does not use Cubism Framework.
namespace {
int physics_type(const std::string& type) {
    if(type=="X") return 0;
    if(type=="Y") return 1;
    if(type=="Angle") return 2;
    return -1;
}
float normalized(float value,float low,float center,float high,float out_low,float out_center,float out_high) {
    value=std::clamp(value,low,high);
    if(value>=center) return out_center+(high>center?(value-center)/(high-center)*(out_high-out_center):0);
    return out_center-(center>low?(center-value)/(center-low)*(out_center-out_low):0);
}
void load_physics(Model& model,const json& definition) {
    if(!definition.contains("PhysicsSettings") || !definition["PhysicsSettings"].is_array()) return;
    const auto& settings=definition["PhysicsSettings"];
    if(settings.size()>256) throw std::runtime_error("Physics setting limit exceeded");
    std::map<std::string,int> ids;
    for(size_t i=0;i<model.ids.size();++i) ids[model.ids[i]]=int(i);
    for(const auto& item:settings) {
        PhysicsSetting rig;
        auto norm=item.value("Normalization",json::object());
        auto pos=norm.value("Position",json::object()), ang=norm.value("Angle",json::object());
        rig.norm_position_min=pos.value("Minimum",-10.0f);
        rig.norm_position_default=pos.value("Default",0.0f);
        rig.norm_position_max=pos.value("Maximum",10.0f);
        rig.norm_angle_min=ang.value("Minimum",-10.0f);
        rig.norm_angle_default=ang.value("Default",0.0f);
        rig.norm_angle_max=ang.value("Maximum",10.0f);
        const auto& inputs=item.value("Input",json::array());
        const auto& outputs=item.value("Output",json::array());
        const auto& vertices=item.value("Vertices",json::array());
        if(inputs.size()>256 || outputs.size()>256 || vertices.size()>64) throw std::runtime_error("Physics chain limit exceeded");
        for(const auto& input:inputs) {
            std::string id=input.at("Source").at("Id").get<std::string>();
            auto found=ids.find(id); int type=physics_type(input.value("Type",""));
            if(found!=ids.end() && type>=0) rig.inputs.push_back({found->second,input.value("Weight",0.0f)*0.01f,type,input.value("Reflect",false)});
        }
        for(const auto& output:outputs) {
            std::string id=output.at("Destination").at("Id").get<std::string>();
            auto found=ids.find(id); int type=physics_type(output.value("Type",""));
            int vertex=output.value("VertexIndex",0);
            if(found!=ids.end() && type>=0 && vertex>0 && vertex<int(vertices.size()))
                rig.outputs.push_back({found->second,vertex,type,output.value("Scale",0.0f),output.value("Weight",0.0f)*0.01f,output.value("Reflect",false)});
        }
        for(const auto& vertex:vertices) {
            auto v=vertex.value("Position",json::object());
            Vec2 p{v.value("X",0.0f),v.value("Y",0.0f)};
            rig.particles.push_back({p,p,p,vertex.value("Mobility",0.0f),vertex.value("Delay",0.0f),vertex.value("Acceleration",0.0f),vertex.value("Radius",0.0f)});
        }
        if(rig.particles.size()>1) model.physics.push_back(std::move(rig));
    }
}
void step_physics(Model& m,float dt) {
    if(dt<=0) return;
    auto* values=csmGetParameterValues(m.core);
    auto* minima=csmGetParameterMinimumValues(m.core);
    auto* maxima=csmGetParameterMaximumValues(m.core);
    auto* defaults=csmGetParameterDefaultValues(m.core);
    constexpr float radians=0.0174532925199433f;
    // Run the same bounded 60 Hz simulation at 30, 60, or variable preview FPS.
    // Discard an excessive catch-up interval rather than freezing the UI.
    constexpr float step=1.0f/60.0f;
    for(auto& rig:m.physics) {
        float x=0,y=0,angle=0;
        for(const auto& input:rig.inputs) {
            int i=input.parameter;
            float n=input.type==2
                ? normalized(values[i],minima[i],defaults[i],maxima[i],rig.norm_angle_min,rig.norm_angle_default,rig.norm_angle_max)
                : normalized(values[i],minima[i],defaults[i],maxima[i],rig.norm_position_min,rig.norm_position_default,rig.norm_position_max);
            n=(n-(input.type==2?rig.norm_angle_default:rig.norm_position_default))
                *input.weight*(input.reflect?-1.0f:1.0f);
            if(input.type==0) x+=n;
            else if(input.type==1) y+=n;
            else angle+=n;
        }
        rig.accumulator=std::min(rig.accumulator+dt,step*8.0f);
        const float theta=angle*radians, c=std::cos(theta),s=std::sin(theta);
        while(rig.accumulator+0.000001f>=step) {
            rig.accumulator-=step;
            rig.particles[0].previous=rig.particles[0].position;
            rig.particles[0].position=rig.particles[0].rest+Vec2{x,y};
            for(size_t j=1;j<rig.particles.size();++j) {
                auto& p=rig.particles[j];
                Vec2 before=p.position;
                Vec2 rest=p.rest-rig.particles[j-1].rest;
                Vec2 rotated{rest.x*c-rest.y*s,rest.x*s+rest.y*c};
                Vec2 target=rig.particles[j-1].position+rotated;
                float damping=std::pow(std::clamp(p.mobility,0.0f,0.999f),step*60.0f);
                Vec2 velocity=(p.position-p.previous)*damping;
                float response=std::max(0.01f,p.acceleration)/(1.0f+std::max(0.0f,p.delay));
                float stiffness=std::clamp(1.0f-std::exp(-response*step*60.0f),0.01f,0.8f);
                p.position=p.position+velocity+(target-p.position)*stiffness;
                Vec2 delta=p.position-rig.particles[j-1].position;
                float distance=std::hypot(delta.x,delta.y);
                float radius=std::hypot(rest.x,rest.y);
                if(distance>0.0001f && radius>0) p.position=rig.particles[j-1].position+delta*(radius/distance);
                if(!std::isfinite(p.position.x)||!std::isfinite(p.position.y)) p.position=target;
                p.previous=before;
            }
        }
        for(const auto& output:rig.outputs) {
            const auto& particle=rig.particles[output.vertex];
            const auto& parent=rig.particles[output.vertex-1];
            Vec2 delta=particle.position-parent.position;
            Vec2 rest=particle.rest-rig.particles[output.vertex-1].rest;
            float measure=output.type==0?delta.x-rest.x:(output.type==1?delta.y-rest.y:
                std::atan2(rest.y*delta.x-rest.x*delta.y,rest.x*delta.x+rest.y*delta.y));
            // A soft response limit keeps high-scale authored chains from folding over
            // when several outputs reach their extrema on the same frame.
            float raw=measure*output.scale*output.weight*0.25f*(output.reflect?1.0f:-1.0f);
            int index=output.parameter;
            float range=std::max(maxima[index]-defaults[index],defaults[index]-minima[index]);
            float limit=std::max(0.001f,range*0.8f);
            float value=limit*std::tanh(raw/limit);
            values[index]=std::clamp(values[index]+value,minima[index],maxima[index]);
        }
    }
}
} // namespace

void* l2d_load_model(const char* path) { TRY
    if(!initialized) throw std::runtime_error("Initialize GL bridge first");
    fs::path file=fs::u8path(path);
    if(file.extension()==".moc3") { file.replace_extension(".model3.json"); if(!fs::exists(file)) throw std::runtime_error("A moc3 needs its sibling model3.json and textures"); }
    auto settings=json::parse(read_file(file)); auto refs=settings.at("FileReferences");
    auto mocbytes=read_file(asset_path(file.parent_path(),refs.at("Moc")));
    auto m=std::make_unique<Model>();
    m->moc_memory.allocate(mocbytes.size(),csmAlignofMoc);
    std::memcpy(m->moc_memory.data,mocbytes.data(),mocbytes.size());
    if(!csmHasMocConsistency(m->moc_memory.data,unsigned(mocbytes.size()))) throw std::runtime_error("Purism rejected inconsistent moc3 data");
    m->moc=csmReviveMocInPlace(m->moc_memory.data,unsigned(mocbytes.size()));
    if(!m->moc) throw std::runtime_error("Purism could not load the moc3 version");
    unsigned model_size=csmGetSizeofModel(m->moc);
    if(model_size==0 || model_size>512u*1024u*1024u) throw std::runtime_error("Invalid Purism model size");
    m->model_memory.allocate(model_size,csmAlignofModel);
    m->core=csmInitializeModelInPlace(m->moc,m->model_memory.data,model_size);
    if(!m->core) throw std::runtime_error(std::string("Purism model initialization failed: ")+csmGetErrorString(csmGetMocError(m->moc)));
    if(csmGetOffscreenCount(m->core)>0) throw std::runtime_error("This model needs offscreen compositing, which the studio renderer does not support yet");
    int drawable_count=csmGetDrawableCount(m->core);
    for(int i=0;i<drawable_count;++i) {
        int blend=csmGetDrawableBlendModes(m->core)[i];
        if((blend&0xff)>2 || ((blend>>8)&0xff)!=0) throw std::runtime_error("This model needs extended blend modes not supported by the studio renderer");
    }
    for(const auto& item:refs.at("Textures")) {
        auto p=asset_path(file.parent_path(),item.get<std::string>());
        auto texture=l2d_texture_load(p.u8string().c_str());
        if(!texture) throw std::runtime_error(error);
        m->tex.push_back(texture);
    }
    json parameters=json::array(),drawables=json::array();
    int parameter_count=csmGetParameterCount(m->core);
    auto* ids=csmGetParameterIds(m->core);
    auto* minimum=csmGetParameterMinimumValues(m->core);
    auto* maximum=csmGetParameterMaximumValues(m->core);
    auto* defaults=csmGetParameterDefaultValues(m->core);
    for(int i=0;i<parameter_count;++i) {
        m->ids.emplace_back(ids[i]);
        parameters.push_back({{"id",ids[i]},{"min",minimum[i]},{"max",maximum[i]},{"default",defaults[i]}});
    }
    if(refs.contains("Physics") && !std::getenv("VALKYRIE_DISABLE_PHYSICS")) {
        auto data=read_file(asset_path(file.parent_path(),refs["Physics"]));
        load_physics(*m,json::parse(data));
    }
    csmUpdateModel(m->core);
    for(int i=0;i<drawable_count;++i) {
        drawables.push_back(csmGetDrawableIds(m->core)[i]);
        auto* vertices=csmGetDrawableVertexPositions(m->core)[i];
        int n=csmGetDrawableVertexCounts(m->core)[i],far=0; float max_distance=0;
        for(int j=1;j<n;++j) {
            float dx=vertices[j].X-vertices[0].X,dy=vertices[j].Y-vertices[0].Y,d=dx*dx+dy*dy;
            if(d>max_distance){max_distance=d;far=j;}
        }
        m->anchor_edges.push_back(far);
    }
    csmVector2 canvas_size{},canvas_origin{}; float pixels_per_unit=1;
    csmReadCanvasInfo(m->core,&canvas_size,&canvas_origin,&pixels_per_unit);
    if(pixels_per_unit<=0) throw std::runtime_error("Invalid canvas pixels per unit");
    m->info=json({{"parameters",parameters},{"drawables",drawables},
        {"canvas_width",canvas_size.X/pixels_per_unit},{"canvas_height",canvas_size.Y/pixels_per_unit},
        {"textures",m->tex.size()},{"physics",!m->physics.empty()},{"runtime","Purism Core"}}).dump();
    auto* handle=m.get(); models[handle]=std::move(m); return handle;
FAIL_RETURN(nullptr) }
void l2d_destroy_model(void* p) { models.erase(p); }
const char* l2d_model_info(void* p) { TRY return model(p).info.c_str(); FAIL_RETURN(nullptr) }
int l2d_get_param_count(void* p) { TRY return int(model(p).ids.size()); FAIL_RETURN(-1) }
const char* l2d_get_param_ids(void* p) { TRY response=json(model(p).ids).dump(); return response.c_str(); FAIL_RETURN(nullptr) }
int l2d_set_parameter(void* p,const char* id,float value) { TRY
    auto& m=model(p); auto found=std::find(m.ids.begin(),m.ids.end(),id);
    if(found==m.ids.end() || !std::isfinite(value)) throw std::runtime_error("Unknown parameter or nonfinite value");
    csmGetParameterValues(m.core)[found-m.ids.begin()]=value; return 1;
FAIL_RETURN(0) }
int l2d_set_parameters(void* p,const float* values,int count) { TRY
    auto& m=model(p); if(!values || count!=int(m.ids.size())) throw std::runtime_error("Parameter buffer length mismatch");
    for(int i=0;i<count;++i) {
        if(!std::isfinite(values[i])) throw std::runtime_error("Nonfinite parameter");
        csmGetParameterValues(m.core)[i]=values[i];
    }
    return 1;
FAIL_RETURN(0) }
int l2d_update(void* p,float dt) { TRY
    auto& m=model(p); if(!std::isfinite(dt)||dt<0||dt>1) throw std::runtime_error("Invalid update step");
    step_physics(m,dt); csmResetDrawableDynamicFlags(m.core); csmUpdateModel(m.core);
    if(csmGetLastError(m.core)!=csmError_NoError) throw std::runtime_error(csmGetErrorString(csmGetLastError(m.core)));
    return 1;
FAIL_RETURN(0) }
int l2d_draw(void* p,unsigned fbo,const float* transform) { TRY
    auto& m=model(p); auto& target=canvas(fbo); auto* core=m.core;
    int count=csmGetDrawableCount(core); auto* orders=csmGetRenderOrders(core);
    if(m.order.size()!=size_t(count) || m.last_render_orders.size()!=size_t(count) ||
       !std::equal(orders,orders+count,m.last_render_orders.begin())) {
        m.order.resize(count); std::iota(m.order.begin(),m.order.end(),0);
        std::sort(m.order.begin(),m.order.end(),[&](int a,int b){return orders[a]==orders[b]?a<b:orders[a]<orders[b];});
        m.last_render_orders.assign(orders,orders+count);
    }
    if(mask.width!=target.width || mask.height!=target.height) {destroy(mask);mask=make_canvas(target.width,target.height);}
    bind(target); prepare_mesh(transform); std::vector<int> previous_masks; bool target_bound=true;
    for(int i:m.order) {
        if(!(csmGetDrawableDynamicFlags(core)[i]&csmIsVisible) || csmGetDrawableOpacities(core)[i]<=0) continue;
        int n=csmGetDrawableMaskCounts(core)[i];
        if(n>0) {
            auto* indices=csmGetDrawableMasks(core)[i];
            if(previous_masks.size()!=size_t(n) || !std::equal(indices,indices+n,previous_masks.begin())) {
                bind(mask); glClearColor(0,0,0,0);glClear(GL_COLOR_BUFFER_BIT);
                for(int j=0;j<n;++j) if(indices[j]>=0) drawable(m,indices[j],transform,0,true);
                previous_masks.assign(indices,indices+n); target_bound=false;
            }
        }
        if(!target_bound) {bind(target);target_bound=true;}
        drawable(m,i,transform,n>0);
    }
    return 1;
FAIL_RETURN(0) }
int l2d_anchor(void* p,int index,float* result) { TRY
    auto& m=model(p); auto* core=m.core;
    if(!result||index<0||index>=csmGetDrawableCount(core)) throw std::runtime_error("Invalid ArtMesh anchor");
    int n=csmGetDrawableVertexCounts(core)[index]; if(n<1) throw std::runtime_error("Empty anchor mesh");
    auto* vertices=csmGetDrawableVertexPositions(core)[index]; float x=0,y=0;
    for(int i=0;i<n;++i){x+=vertices[i].X;y+=vertices[i].Y;}
    auto edge=m.anchor_edges[index]; float dx=vertices[edge].X-vertices[0].X,dy=vertices[edge].Y-vertices[0].Y;
    result[0]=x/n; result[1]=y/n; result[2]=std::atan2(dy,dx);result[3]=std::sqrt(dx*dx+dy*dy); return 1;
FAIL_RETURN(0) }
int l2d_nearest(void* p,float x,float y) { TRY
    auto& m=model(p); auto* core=m.core; float best=1e30f;int selected=-1;
    for(int i=0;i<csmGetDrawableCount(core);++i) {
        if(!(csmGetDrawableDynamicFlags(core)[i]&csmIsVisible)||csmGetDrawableOpacities(core)[i]<0.1) continue;
        float point[4]; if(!l2d_anchor(p,i,point)) continue;
        float dx=x-point[0],dy=y-point[1],distance=dx*dx+dy*dy;
        if(distance<best){best=distance;selected=i;}
    }
    return selected;
FAIL_RETURN(-1) }
